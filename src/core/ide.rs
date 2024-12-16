use eframe::egui::{self, Rect, Stroke, Color32, Painter, Vec2};
use crate::components::{
    file_modal::FileModal,
    code_editor::CodeEditor,
    console_panel::ConsolePanel,
    emulator_panel::EmulatorPanel,
    settings_modal::SettingsModal,
};
use tokio::sync::oneshot;
use crate::core::lsp::LspManager;
use tokio::runtime::Runtime;
use std::sync::{Arc, Mutex};
use lsp_types::{CompletionItem, Diagnostic};

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
    lsp_manager: Arc<Mutex<Option<LspManager>>>,
    tokio_runtime: Arc<Runtime>,
}

impl IDE {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let (shutdown_sender, _shutdown_receiver) = oneshot::channel();
    
        let tokio_runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
        
        let ide = Self {
            file_modal: FileModal::new(),
            code_editor: CodeEditor::new(),
            console_panel: ConsolePanel::new(),
            emulator_panel: EmulatorPanel::new(),
            settings_modal: SettingsModal::new(),
            show_console_panel: false,
            show_emulator_panel: false,
            shutdown_sender: Some(shutdown_sender),
            title: "ZZZ IDE".to_string(),
            lsp_manager: Arc::new(Mutex::new(Some(LspManager::new()))),
            tokio_runtime: tokio_runtime.clone(),
        };
        
        ide.settings_modal.apply_theme(&cc.egui_ctx);
        
        let lsp_manager = ide.lsp_manager.clone();
    
        ide.tokio_runtime.spawn(async move {
            if let Some(lsp_manager) = lsp_manager.lock().unwrap().as_mut() {
                match lsp_manager.start_server() {
                    Ok(_) => println!("IDE: LSP Server started successfully"),
                    Err(e) => eprintln!("IDE: Failed to start LSP Server: {}", e),
                }
            }
        });
    
        ide
    }

    fn map_completion_items_to_strings(completions: Vec<CompletionItem>) -> Vec<String> {
        completions
            .into_iter()
            .map(|item| item.label) // Convert CompletionItem to its label (String)
            .collect()
    }

    fn map_diagnostics_to_strings(diagnostics: Vec<Diagnostic>) -> Vec<String> {
        diagnostics
            .into_iter()
            .map(|diag| format!("{}: {}", diag.range.start.line, diag.message))
            .collect()
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
            if i.key_pressed(egui::Key::Space) && i.modifiers.ctrl {
                if let Some(manager) = self.lsp_manager.lock().unwrap().as_mut() {
                    if let Some(completions) = manager.get_completions() {
                        let string_completions: Vec<String> = completions
                            .into_iter()
                            .map(|item| item.label)
                            .collect();

                        self.code_editor.update_completions(string_completions);
                    }
                }        
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
        
        self.handle_keyboard_shortcuts(ctx);
        self.console_panel.update(ctx);
        
        self.file_modal.show(ctx, &mut self.code_editor.code, &mut self.code_editor.current_file, &mut |msg| self.console_panel.log(msg));
        self.emulator_panel.update_from_file_modal(self.file_modal.project_path.clone());
    
        if let Some(lsp_manager) = self.lsp_manager.lock().unwrap().as_mut() {
            if let Some(completions) = lsp_manager.get_completions() {
                self.code_editor.update_completions(Self::map_completion_items_to_strings(completions));
            }
            
            if let Some(diagnostics) = lsp_manager.get_diagnostics() {
                self.code_editor.update_diagnostics(Self::map_diagnostics_to_strings(diagnostics));
            }            
        }
        
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
            let available_height = 760.0; // Total available height for the central panel
            let console_height = 280.0;  // Height of the console panel
            let editor_height = if self.show_console_panel {
                available_height - console_height
            } else {
                available_height
            };
        
            // Set a fixed height for the editor panel
            egui::ScrollArea::vertical()
                .max_height(editor_height)
                .show(ui, |ui| {
                    ui.set_height(editor_height); // Ensures the editor panel height is fixed
                    self.code_editor.show(ui, available_height); // Render the code editor
                });
        
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

        // Stop the LSP server when dropping the IDE
        if let Some(mut lsp_manager) = self.lsp_manager.lock().unwrap().take() {
            lsp_manager.stop_server();
        }
    }
}

impl eframe::App for IDE {
    fn update(&mut self, ctx: &egui::Context, frame: &mut eframe::Frame) {
        self.update(ctx, frame);
    }
}