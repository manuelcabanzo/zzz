use eframe::egui::{self, Rect, Stroke, Color32, Painter, Vec2};
use crate::components::{
    file_modal::FileModal,
    code_editor::CodeEditor,
    console_panel::ConsolePanel,
    emulator_panel::EmulatorPanel,
    settings_modal::SettingsModal,
    ai_assistant::AIAssistant,
};
use crate::core::app_state::AppState;
use tokio::sync::oneshot;
use tokio::runtime::Runtime;
use std::sync::Arc;

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
}

impl IDE {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();
        let tokio_runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
        
        // Load state first
        let state = AppState::load();
        
        // Create emulator panel first and let it initialize
        let emulator_panel = EmulatorPanel::new();
        
        // Create IDE instance with state-derived values
        let mut ide = Self {
            file_modal: FileModal::new(),
            code_editor: CodeEditor::new(),
            console_panel: ConsolePanel::new(),
            emulator_panel,  // Use the pre-initialized panel
            settings_modal: SettingsModal::new(),
            show_console_panel: state.console_panel_visible,
            show_emulator_panel: state.emulator_panel_visible,
            show_ai_panel: state.ai_assistant_panel_visible,
            shutdown_sender: Some(shutdown_sender),
            title: "ZZZ IDE".to_string(),
            tokio_runtime: tokio_runtime.clone(),
            runtime_handle: None,
            ai_assistant: AIAssistant::new(state.ai_api_key.clone(), tokio_runtime.clone()),
        };
        
        // Enter runtime after creation
        let _guard = tokio_runtime.enter();
        
        // Apply remaining state
        state.apply_to_ide(&mut ide);
        
        // Apply theme after state is loaded
        ide.settings_modal.apply_theme(&cc.egui_ctx);
        
        ide
    }

    fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context, _ui:&mut egui::Ui) {
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
        });
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
                    // Send shutdown signal before closing
                    if let Some(sender) = self.shutdown_sender.take() {
                        let _ = sender.send(());
                    }
                    // Request the window to close
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
        // This is where `ui` is automatically available in the closure
        egui::TopBottomPanel::top("title_bar").show(ctx, |ui| {
            self.custom_title_bar(ui);
        });

        egui::SidePanel::right("emulator_panel")
            .default_width(350.0)
            .resizable(false)
            .max_width(350.0)
            .min_width(350.0)            .show_animated(ctx, self.show_emulator_panel, |ui| {
                self.emulator_panel.show(ui);
            });

        if let Some(new_project_path) = self.file_modal.project_path.clone() {
            if self.console_panel.project_path.as_ref() != Some(&new_project_path) {
                self.console_panel.set_project_path(new_project_path);
            }
        }

        self.console_panel.update(ctx);
        self.file_modal.show(ctx, &mut self.code_editor, &mut |msg| self.console_panel.log(msg));
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
            
            // Calculate available height by getting the panel's size
            let available_space = ui.available_size();
            let console_height = if self.show_console_panel { 280.0 } else { 0.0 };
            let editor_height = available_space.y - console_height;
            
            // Remove the ScrollArea and let the CodeEditor handle its own scrolling
            ui.with_layout(
                egui::Layout::top_down(egui::Align::LEFT).with_main_justify(true),
                |ui| {
                    self.code_editor.show(ui, editor_height);
                },
            );
        });
    
        if self.settings_modal.take_api_key_changed() {
            let new_key = self.settings_modal.get_api_key();
            self.ai_assistant.update_api_key(new_key);
            
            // Update app state
            let mut app_state = AppState::load();
            app_state.ai_api_key = self.settings_modal.get_api_key();
            let _ = app_state.save();
        }
        
        // Handle the console panel and settings modal as well
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
        // Save state before shutting down
        let mut state = AppState::default();
        state.update_from_ide(self);
        if let Err(e) = state.save() {
            eprintln!("Failed to save application state: {}", e);
        }
        
        // Original drop implementation...
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
        self.update(ctx, frame);  // Call your own update method (no changes needed here)
    }
}
