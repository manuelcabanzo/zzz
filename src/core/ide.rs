use eframe::egui;
use std::path::PathBuf;
use std::rc::Rc;
use crate::core::file_system::FileSystem;
use rfd::FileDialog;
use crate::components::{
    file_panel::FilePanel,
    code_editor::CodeEditor,
    console_panel::ConsolePanel,
    emulator_panel::EmulatorPanel,
    settings_modal::SettingsModal,
};
use crate::core::lsp_client::LspClient;
use lsp_types::{
    InitializeParams, ClientCapabilities, Url,
    DidChangeTextDocumentParams, VersionedTextDocumentIdentifier,
    TextDocumentContentChangeEvent,
};
use std::sync::Arc;
use crate::core::lsp_server;
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use std::process;

pub struct IDE {
    file_panel: FilePanel,
    code_editor: CodeEditor,
    console_panel: ConsolePanel,
    emulator_panel: EmulatorPanel,
    settings_modal: SettingsModal,
    file_system: Option<Rc<FileSystem>>,
    project_path: Option<PathBuf>,
    show_file_panel: bool,
    show_console_panel: bool,
    show_emulator_panel: bool,
    lsp_client: Option<Arc<LspClient>>,
    runtime: Arc<Runtime>,
    shutdown_sender: Option<oneshot::Sender<()>>,
    title: String,
    is_dragging: bool,
    drag_start: Option<egui::Pos2>,
    window_pos: egui::Pos2,
}

impl IDE {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {

        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();
        let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

        let runtime_clone = Arc::clone(&runtime);
        std::thread::spawn(move || {
            runtime_clone.block_on(async {
                lsp_server::start_lsp_server().await;
            });
        });

        let ide = Self {
            file_panel: FilePanel::new(),
            code_editor: CodeEditor::new(Arc::clone(&runtime)),
            console_panel: ConsolePanel::new(),
            emulator_panel: EmulatorPanel::new(),
            settings_modal: SettingsModal::new(),
            file_system: None,
            project_path: None,
            show_file_panel: false,
            show_console_panel: false,
            show_emulator_panel: false,
            lsp_client: None,
            runtime,
            shutdown_sender: Some(shutdown_sender),
            title: "ZZZ IDE".to_string(),
            is_dragging: false,
            drag_start: None,
            window_pos: egui::Pos2::ZERO,

        };
        ide.settings_modal.apply_theme(&cc.egui_ctx);

        ide
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Num1) && i.modifiers.ctrl {
                self.show_file_panel = !self.show_file_panel;
            }
            if i.key_pressed(egui::Key::Num2) && i.modifiers.ctrl {
                self.show_emulator_panel = !self.show_emulator_panel;
            }
            if i.key_pressed(egui::Key::Num3) && i.modifiers.ctrl {
                self.show_console_panel = !self.show_console_panel;
            }
            if i.key_pressed(egui::Key::M) && i.modifiers.ctrl {
                self.settings_modal.show = !self.settings_modal.show;
            }
            if i.key_pressed(egui::Key::O) && i.modifiers.ctrl {
                self.open_folder();
            }
        });
    }

    fn open_folder(&mut self) {
        if let Some(folder_path) = FileDialog::new().pick_folder() {
            self.project_path = Some(folder_path.clone());
            self.file_system = Some(Rc::new(FileSystem::new(folder_path.to_str().unwrap())));
            self.file_panel.project_path = Some(folder_path.clone());
            self.file_panel.file_system = self.file_system.clone();
            self.file_panel.expanded_folders.clear();
            self.file_panel.expanded_folders.insert(folder_path.clone());
            self.console_panel.log(&format!("Opened project: {}", folder_path.display()));

            // Use the existing runtime to initialize the LSP client
            self.runtime.block_on(async {
                let lsp_client = Arc::new(LspClient::new());

                let root_uri = Url::from_file_path(&folder_path).expect("Failed to create URL from path");
                let init_params = InitializeParams {
                    root_uri: Some(root_uri),
                    capabilities: ClientCapabilities::default(),
                    ..InitializeParams::default()
                };
                match lsp_client.initialize(init_params).await {
                    Ok(_) => {
                        self.lsp_client = Some(lsp_client.clone());
                        self.code_editor.set_lsp_client(lsp_client);
                        self.console_panel.log("LSP client initialized successfully");
                    },
                    Err(e) => {
                        self.console_panel.log(&format!("Failed to initialize LSP client: {:?}", e));
                    }
                }
            });
        }
    }

    fn custom_title_bar(&mut self, ctx: &egui::Context) {
        let title_bar_height = 28.0;
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            ui.set_height(title_bar_height);
            ui.horizontal(|ui| {
                ui.label(&self.title);
                let title_bar_response = ui.allocate_rect(ui.min_rect(), egui::Sense::click_and_drag());

                if title_bar_response.drag_started() {
                    self.is_dragging = true;
                    self.drag_start = ctx.pointer_hover_pos();
                }

                if self.is_dragging {
                    if let (Some(drag_start), Some(current_pos)) = (self.drag_start, ctx.pointer_hover_pos()) {
                        let delta = current_pos - drag_start;
                        self.window_pos += delta;
                        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(ctx.screen_rect().size()));
                        ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(self.window_pos));
                        self.drag_start = Some(current_pos);
                    }
                }

                if title_bar_response.drag_stopped() {
                    self.is_dragging = false;
                    self.drag_start = None;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("❌").clicked() {
                        process::exit(0);
                    }
                    if ui.button("🗖").clicked() {
                        let is_maximized = ctx.input(|i| i.viewport().maximized.unwrap_or(false));
                        ctx.send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                    }
                    if ui.button("🗕").clicked() {
                        ctx.send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                    }
                });
            });
        });
    }
    
    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        
        self.custom_title_bar(ctx);  // Add this line
        self.handle_keyboard_shortcuts(ctx);
        self.console_panel.update();
        
        if self.show_file_panel {
            self.file_panel.show(ctx, &mut self.code_editor.code, &mut self.code_editor.current_file, &mut |msg| self.console_panel.log(msg));
        }

        if self.show_emulator_panel {
            self.emulator_panel.show(ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| {    
            let fixed_editor_height = 730.0;    
            let available_height = if self.show_console_panel {         
                (ui.available_height() - 280.0).min(fixed_editor_height)    
            } else {        
                fixed_editor_height    
            };    
            self.code_editor.show(ui, available_height);

            if let Some(lsp_client) = &self.lsp_client {
                if let Some(current_file) = &self.code_editor.current_file {
                    let uri = Url::from_file_path(current_file).expect("Failed to create URL from path");
                    let lsp_client_clone = Arc::clone(lsp_client);
                    let code = self.code_editor.code.clone();
                    self.runtime.spawn(async move {
                        lsp_client_clone.did_change(DidChangeTextDocumentParams {
                            text_document: VersionedTextDocumentIdentifier {
                                uri: uri.clone(),
                                version: 0, // You might want to implement proper versioning
                            },
                            content_changes: vec![TextDocumentContentChangeEvent {
                                range: None,
                                range_length: None,
                                text: code,
                            }],
                        }).await;
                    });
                }
            }        
        });

        if self.show_console_panel {
            self.console_panel.show(ctx);
        }

        self.settings_modal.show(ctx);
    }
}

impl Drop for IDE {
    fn drop(&mut self) {
        if let Some(sender) = self.shutdown_sender.take() {
            let _ = sender.send(());
        }
    }
}

impl eframe::App for IDE {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update(ctx, frame);
        self.settings_modal.show(ctx);
    }
}
