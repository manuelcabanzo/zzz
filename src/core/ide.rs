use eframe::egui::{self, Rect, Stroke, Color32, Painter, Vec2, TextEdit, ScrollArea};
use crate::components::{
    file_modal::FileModal,
    code_editor::CodeEditor,
    console_panel::ConsolePanel,
    emulator_panel::EmulatorPanel,
    settings_modal::SettingsModal,
    ai_assistant::AIAssistant,
    git_modal::GitModal,
};
use crate::core::app_state::AppState;
use tokio::sync::oneshot;
use tokio::runtime::Runtime;
use std::sync::{Arc, Mutex};
use std::path::Path;
use std::fs;
use super::git_manager::GitManager;
use super::search::{show_search_modal, SearchResult};
use crate::plugin_manager::PluginManager;

pub struct IDE {
    pub file_modal: FileModal,
    pub code_editor: CodeEditor,
    pub console_panel: ConsolePanel,
    pub emulator_panel: EmulatorPanel,
    pub settings_modal: SettingsModal,
    pub show_console_panel: bool,
    pub show_emulator_panel: bool,
    pub show_ai_panel: bool,
    pub shutdown_sender: Option<oneshot::Sender<()>>,
    title: String,
    pub tokio_runtime: Arc<Runtime>,
    runtime_handle: Option<tokio::task::JoinHandle<()>>,
    pub ai_assistant: AIAssistant,
    pub show_file_search_modal: bool,
    pub file_search_query: String,
    pub file_search_results: Vec<String>,
    pub show_current_file_search_modal: bool,
    pub show_project_search_modal: bool,
    pub search_query: String,
    pub search_results: Vec<SearchResult>,
    pub search_highlight_text: Option<String>,
    pub search_focus_requested: bool,
    pub ai_model: String,
    pub git_modal: GitModal,
    pub plugin_manager: Arc<Mutex<PluginManager>>,
}

impl IDE {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();
        let tokio_runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));

        let state = AppState::load();
        let emulator_panel = EmulatorPanel::new();

        let mut code_editor = CodeEditor::new();
        if let Err(err) = code_editor.load_logo(&cc.egui_ctx) {
            eprintln!("Failed to load logo: {}", err);
        }

        let plugin_manager = PluginManager::new();
        let plugin_manager_arc = Arc::new(Mutex::new(plugin_manager));
        let file_modal = FileModal::new();

        let mut ide = Self {
            file_modal: file_modal,
            code_editor,
            console_panel: ConsolePanel::new(),
            emulator_panel,
            settings_modal: SettingsModal::new(plugin_manager_arc.clone()),
            show_console_panel: state.console_panel_visible,
            show_emulator_panel: state.emulator_panel_visible,
            show_ai_panel: state.ai_assistant_panel_visible,
            shutdown_sender: Some(shutdown_sender),
            title: "ZZZ IDE".to_string(),
            tokio_runtime: tokio_runtime.clone(),
            runtime_handle: None,
            ai_assistant: AIAssistant::new(state.ai_api_key.clone(), tokio_runtime.clone()),
            show_file_search_modal: false,
            file_search_query: String::new(),
            file_search_results: Vec::new(),
            show_current_file_search_modal: false,
            show_project_search_modal: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_highlight_text: None,
            search_focus_requested: false,
            ai_model: state.ai_model.clone(),
            git_modal: GitModal::new(tokio_runtime.clone()),
            plugin_manager: plugin_manager_arc.clone(),
        };

        let _guard = tokio_runtime.enter();

        if let Some(project_path) = &ide.file_modal.project_path {
            println!("Project path: {}", project_path.display());
            let git_manager = GitManager::new(project_path.clone());

            match git_manager.initialize() {
                Ok(_) => {
                    ide.console_panel.log("Git repository initialized successfully");
                },
                Err(e) => {
                    ide.console_panel.log(&format!("Git initialization error: {}", e));
                }
            }
        }
        state.apply_to_ide(&mut ide);
        ide.settings_modal.apply_theme(&cc.egui_ctx);
        ide.load_extensions();
        ide.load_plugins();

        ide
    }

    fn load_extensions(&mut self) {
        // Add logic to discover and load extensions from filesystem
        let extensions_dir = Path::new("extensions");
        if extensions_dir.exists() && extensions_dir.is_dir() {
            for entry in fs::read_dir(extensions_dir).expect("Failed to read extensions directory") {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        // Load the extension
                        // self.extension_manager.load_extension(Box::new(ExampleExtension));
                        println!("Loaded extension from path: {:?}", path);
                    }
                }
            }
        }
    }

    fn load_plugins(&mut self) {
        // Add logic to discover and load plugins from filesystem
        let plugins_dir = Path::new("plugins");
        if plugins_dir.exists() && plugins_dir.is_dir() {
            for entry in fs::read_dir(plugins_dir).expect("Failed to read plugins directory") {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.is_file() {
                        // Load the plugin
                        self.plugin_manager.lock().unwrap().install_plugin(&path);
                        println!("Loaded plugin from path: {:?}", path);
                    }
                }
            }
        }
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, _ui: &mut egui::Ui) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::Num1) && i.modifiers.ctrl {
                self.file_modal.show = !self.file_modal.show;
            }
            if i.key_pressed(egui::Key::Num2) && i.modifiers.ctrl {
                self.show_emulator_panel = !self.show_emulator_panel;
            }
            if i.key_pressed(egui::Key::Num3) && i.modifiers.ctrl {
                self.show_ai_panel = !self.show_ai_panel;
            }
            if i.key_pressed(egui::Key::Num4) && i.modifiers.ctrl {
                self.show_console_panel = !self.show_console_panel;
            }
            if i.key_pressed(egui::Key::M) && i.modifiers.ctrl {
                self.settings_modal.show = !self.settings_modal.show;
            }
            if i.key_pressed(egui::Key::O) && i.modifiers.ctrl {
                self.file_modal.open_folder(&mut |msg| self.console_panel.log(msg));
            }
            if i.key_pressed(egui::Key::S) && i.modifiers.ctrl {
                self.file_modal.save_current_file(&mut self.code_editor, &mut |msg| self.console_panel.log(msg));
            }
            if i.key_pressed(egui::Key::P) && i.modifiers.ctrl {
                self.show_file_search_modal = true;
            }
            if i.key_pressed(egui::Key::F) && i.modifiers.ctrl && !i.modifiers.shift {
                if self.show_current_file_search_modal {
                    self.show_current_file_search_modal = false;
                } else if self.code_editor.get_active_buffer().is_some() {
                    self.show_current_file_search_modal = true;
                    self.search_query = String::new();
                    self.search_results = Vec::new();
                    self.search_focus_requested = true;
                }
            }
            if i.key_pressed(egui::Key::F) && i.modifiers.ctrl && i.modifiers.shift {
                if self.show_project_search_modal {
                    self.show_project_search_modal = false;
                } else {
                    self.show_project_search_modal = true;
                    self.search_query = String::new();
                    self.search_results = Vec::new();
                    self.search_focus_requested = true;
                }
            }
            if i.key_pressed(egui::Key::G) && i.modifiers.ctrl {
                self.git_modal.show = !self.git_modal.show;
                if self.git_modal.show {
                    self.git_modal.update_git_manager(self.file_modal.project_path.clone());
                }
            }
            if i.key_pressed(egui::Key::Escape) {
                self.show_current_file_search_modal = false;
                self.show_project_search_modal = false;
            }
        });
    }

    fn custom_title_bar(&mut self, ui: &mut egui::Ui) {
        let title_bar_height = 28.0;
        let button_size = egui::vec2(title_bar_height * 0.4, title_bar_height * 0.4);
        ui.set_height(title_bar_height);

        ui.horizontal(|ui| {
            ui.label(&self.title);
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));
                if self.draw_title_button(ui, button_size, |painter, rect, color| {
                    let line_start1 = rect.left_top() + Vec2::new(1.8, 1.8);
                    let line_end1 = rect.right_bottom() - Vec2::new(1.8, 1.8);
                    let line_start2 = rect.right_top() + Vec2::new(-1.8, 1.8);
                    let line_end2 = rect.left_bottom() + Vec2::new(1.8, -1.8);
                    painter.line_segment([line_start1, line_end1], Stroke::new(1.3, color));
                    painter.line_segment([line_start2, line_end2], Stroke::new(1.3, color));
                }).clicked() {
                    if let Some(sender) = self.shutdown_sender.take() {
                        let _ = sender.send(());
                    }
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }
                if self.draw_title_button(ui, button_size, |painter, rect, color| {
                    if is_maximized {
                        let small_rect = Rect::from_min_size(
                            rect.left_top() + Vec2::new(2.0, 2.0),
                            Vec2::new(rect.width() - 4.0, rect.height() - 4.0),
                        );
                        painter.rect_stroke(small_rect, 0.0, Stroke::new(0.5, color));
                        painter.line_segment(
                            [small_rect.left_top() + Vec2::new(-1.0, -1.0), small_rect.right_top() + Vec2::new(-1.0, -1.0)],
                            Stroke::new(0.5, color),
                        );
                        painter.line_segment(
                            [small_rect.left_top() + Vec2::new(-1.0, -1.0), small_rect.left_bottom() + Vec2::new(-1.0, -1.0)],
                            Stroke::new(0.5, color),
                        );
                    } else {
                        painter.rect_stroke(rect.shrink(1.5), 0.0, Stroke::new(0.5, color));
                    }
                }).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }
                if self.draw_title_button(ui, button_size, |painter, rect, color| {
                    let line_start = rect.left_center() + Vec2::new(1.8, 0.0);
                    let line_end = rect.right_center() + Vec2::new(-1.8, 0.0);
                    painter.line_segment([line_start, line_end], Stroke::new(0.5, color));
                }).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Minimized(true));
                }
            });
        });
    }

    fn draw_title_button<F>(&self, ui: &mut egui::Ui, size: egui::Vec2, draw_func: F) -> egui::Response
    where
        F: FnOnce(&Painter, Rect, Color32),
    {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.style().noninteractive();
            let base_color = visuals.fg_stroke.color;

            let color = if response.clicked() {
                self.adjust_color(base_color, -30)
            } else if response.hovered() {
                self.adjust_color(base_color, 40)
            } else {
                base_color
            };
            let painter = ui.painter();
            draw_func(&painter, rect, color);
        }
        response
    }

    fn adjust_color(&self, color: Color32, amount: i16) -> Color32 {
        let [r, g, b, a] = color.to_array();
        Color32::from_rgba_unmultiplied(
            (r as i16 + amount).clamp(0, 255) as u8,
            (g as i16 + amount).clamp(0, 255) as u8,
            (b as i16 + amount).clamp(0, 255) as u8,
            a,
        )
    }

    fn show_file_search_modal(&mut self, ctx: &egui::Context) {
        if self.show_file_search_modal {
            egui::Window::new("File Search")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        let text_edit = TextEdit::singleline(&mut self.file_search_query).hint_text("Type to search...");

                        let response = ui.add(text_edit);
                        if response.gained_focus() {
                            response.request_focus();
                        }

                        if !self.file_search_query.is_empty() {
                            self.file_search_results = self.file_modal.search_files(&self.file_search_query);
                        }

                        ScrollArea::vertical().show(ui, |ui| {
                            for result in &self.file_search_results {
                                if ui.button(result).clicked() {
                                    self.file_modal.open_file(result, &mut self.code_editor);
                                    self.show_file_search_modal = false;
                                }
                            }
                        });
                    });
                });
        }
    }

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            self.custom_title_bar(ui);
        });

        egui::SidePanel::right("emulator_panel")
            .default_width(350.0)
            .resizable(false)
            .max_width(350.0)
            .min_width(350.0)
            .show_animated(ctx, self.show_emulator_panel, |ui| {
                self.emulator_panel.show(ui);
            });

        self.code_editor.clear_expired_highlights();


        ctx.data_mut(|data| {
            let frame_count = data.get_temp::<u32>(egui::Id::new("frame_count")).unwrap_or(0);
            let should_update = frame_count % 3 == 0;
            data.insert_temp(egui::Id::new("frame_count"), frame_count + 1);

            if !should_update {
                return;
            }
        });

        if let Some(new_project_path) = self.file_modal.project_path.clone() {
            if self.console_panel.project_path.as_ref() != Some(&new_project_path) {
                self.console_panel.set_project_path(new_project_path);
            }
        }

        show_search_modal(self, ctx);
        self.console_panel.update(ctx);
        self.file_modal.show(ctx, &mut self.code_editor, &mut |msg| self.console_panel.log(msg), &mut self.ai_assistant);
        self.emulator_panel.update_from_file_modal(self.file_modal.project_path.clone());

        if self.show_ai_panel {
            egui::SidePanel::right("ai_panel")
                .default_width(350.0)
                .resizable(false)
                .max_width(350.0)
                .min_width(350.0)
                .show_animated(ctx, self.show_ai_panel, |ui| {
                    self.ai_assistant.show(ui, &mut self.code_editor);
                });
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            self.handle_keyboard_shortcuts(ctx, ui);
            let available_space = ui.available_size();
            let console_height = if self.show_console_panel { 280.0 } else { 0.0 };
            let editor_height = available_space.y - console_height;

            ui.with_layout(
                egui::Layout::top_down(egui::Align::LEFT).with_main_justify(true),
                |ui| {
                    let editor_id = ui.id().with("code_editor");
                    if !ctx.is_being_dragged(editor_id) {
                        self.code_editor.show(ui, editor_height);
                    }
                },
            );
        });

        if self.settings_modal.take_api_key_changed() {
            let new_key = self.settings_modal.get_api_key();
            self.ai_assistant.update_api_key(new_key);

            let mut app_state = AppState::load();
            app_state.ai_api_key = self.settings_modal.get_api_key();
            let _ = app_state.save();
        }

        if self.settings_modal.take_ai_model_changed() {
            let new_model = self.settings_modal.get_ai_model();
            self.ai_model = new_model.clone();
            self.ai_assistant.update_model(new_model);

            let mut app_state = AppState::load();
            app_state.ai_model = self.settings_modal.get_ai_model();
            let _ = app_state.save();
        }

        if self.show_console_panel {
            egui::TopBottomPanel::bottom("console_panel")
                .resizable(false)
                .exact_height(280.0)
                .show(ctx, |ui| {
                    self.console_panel.show(ui);
                });
        }

        self.settings_modal.show(ctx);
        self.show_file_search_modal(ctx);
        self.git_modal.show(
            ctx,
            &mut self.file_modal,
            &mut self.code_editor,
            &mut self.console_panel
        );    
    }
}

impl Drop for IDE {
    fn drop(&mut self) {
        let mut state = AppState::default();
        state.update_from_ide(self);
        if let Err(e) = state.save() {
            eprintln!("Failed to save application state: {}", e);
        }

        if let Some(handle) = self.runtime_handle.take() {
            handle.abort();
        }

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