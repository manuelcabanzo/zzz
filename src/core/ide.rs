use eframe::egui::{self, Rect, Stroke, Color32, Painter, Vec2};
use crate::components::{
    file_modal::FileModal,
    code_editor::CodeEditor,
    console_panel::ConsolePanel,
    emulator_panel::EmulatorPanel,
    settings_modal::SettingsModal,
};
use tokio::runtime::Runtime;
use tokio::sync::oneshot;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use crate::components::emulator_panel::AppState;

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
    app_state: Arc<AppState>,
}

impl IDE {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();

        let app_state = Arc::new(AppState::new());

        let ide = Self {
            file_modal: FileModal::new(Arc::clone(&runtime)),
            code_editor: CodeEditor::new(Arc::clone(&runtime)),
            console_panel: ConsolePanel::new(),
            emulator_panel: EmulatorPanel::new(Arc::clone(&app_state)),
            settings_modal: SettingsModal::new(),
            show_console_panel: false,
            show_emulator_panel: false,
            shutdown_sender: Some(shutdown_sender),
            title: "ZZZ IDE".to_string(),
            lsp_initialized: AtomicBool::new(false),
            runtime,
            app_state,
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
        let button_size = egui::vec2(title_bar_height * 0.4, title_bar_height * 0.4); // Reduced button size
        ui.set_height(title_bar_height);
        
        ui.horizontal(|ui| {
            ui.label(&self.title);

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let is_maximized = ui.ctx().input(|i| i.viewport().maximized.unwrap_or(false));

                // Close button (X)
                if self.draw_title_button(ui, button_size, |painter, rect, color| {
                    let line_start1 = rect.left_top() + Vec2::new(1.8, 1.8);
                    let line_end1 = rect.right_bottom() - Vec2::new(1.8, 1.8);
                    let line_start2 = rect.right_top() + Vec2::new(-1.8, 1.8);
                    let line_end2 = rect.left_bottom() + Vec2::new(1.8, -1.8);
                    painter.line_segment([line_start1, line_end1], Stroke::new(1.3, color));
                    painter.line_segment([line_start2, line_end2], Stroke::new(1.3, color));
                }).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                }

                // Maximize/Restore button
                if self.draw_title_button(ui, button_size, |painter, rect, color| {
                    if is_maximized {
                        // Draw a "restore down" icon
                        let small_rect = Rect::from_min_size(
                            rect.left_top() + Vec2::new(2.0, 2.0),
                            Vec2::new(rect.width() - 4.0, rect.height() - 4.0)
                        );
                        painter.rect_stroke(small_rect, 0.0, Stroke::new(0.5, color));
                        painter.line_segment(
                            [small_rect.left_top() + Vec2::new(-1.0, -1.0), small_rect.right_top() + Vec2::new(-1.0, -1.0)],
                            Stroke::new(0.5, color)
                        );
                        painter.line_segment(
                            [small_rect.left_top() + Vec2::new(-1.0, -1.0), small_rect.left_bottom() + Vec2::new(-1.0, -1.0)],
                            Stroke::new(0.5, color)
                        );
                    } else {
                        // Draw a "maximize" icon (simple square)
                        painter.rect_stroke(rect.shrink(1.5), 0.0, Stroke::new(0.5, color));
                    }
                }).clicked() {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Maximized(!is_maximized));
                }

                // Minimize button (-)
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
    where F: FnOnce(&Painter, Rect, Color32) {
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
            a
        )
    }

    pub fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            self.custom_title_bar(ui);
        });
        
        self.initialize_lsp();
        self.handle_keyboard_shortcuts(ctx);
        self.console_panel.update();
        
        self.file_modal.show(ctx, &mut self.code_editor.code, &mut self.code_editor.current_file, &mut |msg| self.console_panel.log(msg));

        egui::SidePanel::right("emulator_panel")
            .resizable(true)
            .default_width(250.0)
            .show_animated(ctx, self.show_emulator_panel, |ui| {
                self.emulator_panel.show(ui);
            });

        if let Some(new_project_path) = self.file_modal.project_path.clone() {
            if self.console_panel.project_path.as_ref() != Some(&new_project_path) {
                self.console_panel.set_project_path(new_project_path);
            }
        }
        
        egui::CentralPanel::default().show(ctx, |ui| {    
            let available_height = 715.0;
            let console_height = 280.0;
            let editor_height = if self.show_console_panel {
                available_height - console_height
            } else {
                available_height
            };
            
            self.code_editor.show(ui, editor_height);

            if let Some(current_file) = &self.code_editor.current_file {
                let code = self.code_editor.code.clone();
                self.file_modal.notify_file_change(current_file, &code);
                
                // Update app_state based on code changes
                let mut background_color = self.app_state.background_color.lock().unwrap();
                *background_color = if code.contains("background-color: red;") {
                    Color32::RED
                } else if code.contains("background-color: blue;") {
                    Color32::BLUE
                } else {
                    Color32::WHITE
                };
                
                // Update content
                if let Some(content_start) = code.find("content: \"") {
                    if let Some(content_end) = code[content_start + 9..].find("\"") {
                        let new_content = code[content_start + 9..content_start + 9 + content_end].to_string();
                        let mut content = self.app_state.content.lock().unwrap();
                        *content = new_content;
                    }
                }
            }  
        });

        if self.show_console_panel {
            egui::TopBottomPanel::bottom("console_panel")
                .resizable(false)
                .exact_height(280.0)
                .show_animated(ctx, self.show_console_panel, |ui| {
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