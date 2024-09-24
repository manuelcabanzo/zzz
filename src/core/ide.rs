use eframe::egui;
use crate::components::{
    file_modal::FileModal,
    code_editor::CodeEditor,
    console_panel::ConsolePanel,
    emulator_panel::EmulatorPanel,
    settings_modal::SettingsModal,
};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use std::sync::{Arc, Mutex};
use crate::core::terminal::Terminal;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct IDE {
    file_modal: FileModal,
    code_editor: CodeEditor,
    console_panel: ConsolePanel,
    emulator_panel: EmulatorPanel,
    settings_modal: SettingsModal,
    show_console_panel: bool,
    show_emulator_panel: bool,
    shutdown_sender: Option<oneshot::Sender<()>>,
    title: String,
    lsp_initialized: AtomicBool,
    runtime: Arc<Runtime>, 
}

impl IDE {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (terminal, output_receiver) = Terminal::new();
        let terminal = Arc::new(Mutex::new(terminal));
        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();
        let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

        let ide = Self {
            file_modal: FileModal::new(Arc::clone(&runtime), Arc::clone(&terminal)),
            code_editor: CodeEditor::new(Arc::clone(&runtime)),
            console_panel: ConsolePanel::new(Arc::clone(&terminal), output_receiver),
            emulator_panel: EmulatorPanel::new(),
            settings_modal: SettingsModal::new(),
            show_console_panel: false,
            show_emulator_panel: false,
            shutdown_sender: Some(shutdown_sender),
            title: "ZZZ IDE".to_string(),
            lsp_initialized: AtomicBool::new(false),
            runtime, // Add this line
        };
        
        ide.settings_modal.apply_theme(&cc.egui_ctx);

        ide
    }
    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Num1) && i.modifiers.ctrl {
                self.file_modal.show = !self.file_modal.show;
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
                self.file_modal.open_folder(&mut |msg| self.console_panel.log(msg));
            }
        });
    }

    fn initialize_lsp(&self) {
        if self.lsp_initialized.compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst).is_ok() {
            let runtime_clone = Arc::clone(&self.runtime);
            std::thread::spawn(move || {
                runtime_clone.block_on(async {
                    crate::core::lsp_server::start_lsp_server().await;
                });
            });
        }
    }

    fn custom_title_bar(&mut self, ui: &mut egui::Ui) {
        let title_bar_height = 28.0;
        ui.set_height(title_bar_height);
        ui.horizontal(|ui| {
            ui.label(&self.title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("❌").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if ui.button("🗖").clicked() {
                    let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }
                if ui.button("🗕").clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });
    }

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            self.custom_title_bar(ui);
        });
        
        self.initialize_lsp();
        self.handle_keyboard_shortcuts(ctx);
        self.console_panel.update();
        
        self.file_modal.show(ctx, &mut self.code_editor.code, &mut self.code_editor.current_file, &mut |msg| self.console_panel.log(msg));

        if self.show_emulator_panel {
            self.emulator_panel.show(ctx);
        }

        egui::CentralPanel::default().show(ctx, |ui| {    
            let available_height = 715.0; 
            let console_height = 280.0; // Fixed console height
            let editor_height = if self.show_console_panel {
                available_height - console_height
            } else {
                available_height
            };
            
            self.code_editor.show(ui, editor_height);

            if let Some(current_file) = &self.code_editor.current_file {
                let code = self.code_editor.code.clone();
                self.file_modal.notify_file_change(current_file, &code);
            }      
        });

        if self.show_console_panel {
            egui::TopBottomPanel::bottom("console_panel").resizable(false).exact_height(280.0).show(ctx, |ui| {
                self.console_panel.show(ui);
            });
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
    }
}
