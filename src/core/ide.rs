use eframe::egui::{self, Rect, Stroke, Color32, Painter, Vec2, TextEdit, ScrollArea}; // Add TextEdit and ScrollArea
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
use std::rc::Rc;
use crate::core::file_system::FileSystem;
use std::path::Path;

#[derive(Clone)]
pub struct SearchResult {
    pub line_number: usize,
    pub line_content: String,
    pub file_path: Option<String>,
}

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
    search_focus_requested: bool, // Add this field
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
            show_file_search_modal: false,
            file_search_query: String::new(),
            file_search_results: Vec::new(),
            show_current_file_search_modal: false,
            show_project_search_modal: false,
            search_query: String::new(),
            search_results: Vec::new(),
            search_highlight_text: None,
            search_focus_requested: false, // Initialize the field
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
                    self.search_focus_requested = true; // Request focus
                }
            }
            if i.key_pressed(egui::Key::F) && i.modifiers.ctrl && i.modifiers.shift {
                if self.show_project_search_modal {
                    self.show_project_search_modal = false;
                } else {
                    self.show_project_search_modal = true;
                    self.search_query = String::new();
                    self.search_results = Vec::new();
                    self.search_focus_requested = true; // Request focus
                }
            }
            if i.key_pressed(egui::Key::Escape) {
                self.show_current_file_search_modal = false;
                self.show_project_search_modal = false;
            }
        });
    }

    fn perform_current_file_search(&mut self) {
        if let Some(buffer) = self.code_editor.get_active_buffer() {
            let content = &buffer.content;
            self.search_results = content
                .lines()
                .enumerate()
                .filter(|(_, line)| line.contains(&self.search_query))
                .map(|(line_num, line)| SearchResult {
                    line_number: line_num + 1,
                    line_content: line.to_string(),
                    file_path: buffer.file_path.clone(), // Add this line
                })
                .collect();
        }
    }
    
    fn perform_project_search(&mut self) {
        const MAX_RESULTS: usize = 100;
        let mut results = Vec::new();
        
        // Kotlin/Mobile-specific excluded directories
        let excluded_dirs = vec![
            // Build and compilation directories
            "build", 
            "target", 
            "out",
            "bin",
            
            // Dependency and cache directories
            "node_modules",
            ".gradle",
            "gradle",
            "captures",
            
            // Version control and IDE-specific
            ".git", 
            ".svn", 
            ".idea", 
            ".vscode",
            
            // Android-specific
            "app/build",
            "androidTest",
            "test",
            "debug",
            "release",
            
            // Kotlin/Multiplatform specific
            "shared/build",
            "commonMain",
            "androidMain",
            "iosMain",
            
            // Misc system and cache folders
            "__MACOSX",
            ".DS_Store",
            "*.xcodeproj",
            "*.iml",
        ];
        
        if let Some(fs) = &self.file_modal.file_system {
            if let Some(project_path) = &self.file_modal.project_path {
                let query = self.search_query.trim();
                if query.len() >= 2 {
                    self.search_in_directory_with_exclusions(
                        fs, 
                        project_path, 
                        query, 
                        &mut results, 
                        MAX_RESULTS,
                        &excluded_dirs
                    );
                }
            }
        }
        self.search_results = results;
    }
    
    fn search_in_directory_with_exclusions(
        &self, 
        fs: &Rc<FileSystem>, 
        dir: &Path, 
        query: &str, 
        results: &mut Vec<SearchResult>, 
        max_results: usize,
        excluded_dirs: &[&str]
    ) {
        if results.len() >= max_results {
            return;
        }
    
        if let Ok(entries) = fs.list_directory(dir) {
            for entry in entries {
                if results.len() >= max_results {
                    break;
                }
    
                let path = dir.join(&entry.name);
                
                // Skip file types and excluded directories
                if entry.is_dir {
                    let dir_name = path.file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("");
                    
                    // Pattern matching for more flexible exclusion
                    if excluded_dirs.iter().any(|&excluded| 
                        dir_name == excluded || 
                        dir_name.starts_with(excluded) || 
                        dir_name.contains(excluded)
                    ) {
                        continue;
                    }
                    
                    // Recursively search non-excluded directories
                    self.search_in_directory_with_exclusions(
                        fs, 
                        &path, 
                        query, 
                        results, 
                        max_results, 
                        excluded_dirs
                    );
                } else {
                    // Skip large or binary files
                    let file_ext = path.extension()
                        .and_then(|ext| ext.to_str())
                        .unwrap_or("");
                    
                    let skippable_extensions = [
                        "png", "jpg", "jpeg", "gif", 
                        "svg", "pdf", "zip", 
                        "tar", "gz", "class", 
                        "jar", "so", "dll", 
                        "dylib", "o", "a"
                    ];
                    
                    if skippable_extensions.contains(&file_ext) {
                        continue;
                    }
                    
                    // File content search
                    if let Ok(content) = fs.open_file(&path) {
                        let file_results = content
                            .lines()
                            .enumerate()
                            .filter(|(_, line)| line.contains(query))
                            .take(10) // Limit matches per file
                            .map(|(line_num, line)| SearchResult {
                                line_number: line_num + 1,
                                line_content: line.to_string(),
                                file_path: Some(path.to_str().unwrap().to_string()),
                            })
                            .collect::<Vec<_>>();
                        
                        results.extend(file_results);
                    }
                }
            }
        }
    }

    fn show_search_modal(&mut self, ctx: &egui::Context) {
        let is_project_search = self.show_project_search_modal;
        let modal_title = if is_project_search { "Project Search" } else { "Current File Search" };
    
        if self.show_current_file_search_modal || self.show_project_search_modal {
            egui::Window::new(modal_title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        let text_edit = TextEdit::singleline(&mut self.search_query)
                            .hint_text("Search...");
                        
                        // Automatically request focus when the modal is opened
                        let response = ui.add(text_edit);
                        if self.search_focus_requested {
                            response.request_focus();
                            self.search_focus_requested = false; // Reset the flag
                        }
                        
                        if !self.search_query.is_empty() {
                            if is_project_search {
                                self.perform_project_search();
                            } else {
                                self.perform_current_file_search();
                            }
                        }
    
                        ScrollArea::vertical().show(ui, |ui| {
                            for result in self.search_results.iter() {
                                let display_text = if is_project_search {
                                    format!(
                                        "{}:{} - {}",
                                        result.file_path.as_ref().unwrap_or(&"Unknown".to_string()),
                                        result.line_number,
                                        result.line_content.trim()
                                    )
                                } else {
                                    format!("Line {}: {}", result.line_number, result.line_content.trim())
                                };
    
                                let response = ui.button(display_text);
                                if response.clicked() || ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                    // Handle file opening and cursor positioning
                                    if is_project_search {
                                        if let Some(file_path) = &result.file_path {
                                            self.file_modal.open_file(file_path, &mut self.code_editor);
                                        }
                                    }
                                    
                                    // Set cursor position in the active buffer
                                    if let Some(buffer) = self.code_editor.get_active_buffer_mut() {
                                        buffer.set_cursor_position(
                                            result.line_number,
                                            result.line_content.find(&self.search_query).unwrap_or(0)
                                        );
                                    }
                                    
                                    // Perform search and highlighting in the code editor
                                    self.code_editor.search(
                                        &self.search_query,
                                        Some(result.line_number)
                                    );
    
                                    // Close the search modal
                                    self.show_current_file_search_modal = false;
                                    self.show_project_search_modal = false;
                                }
                            }
                        });
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

    fn show_file_search_modal(&mut self, ctx: &egui::Context) {
        if self.show_file_search_modal {
            egui::Window::new("File Search")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.vertical(|ui| {
                        let text_edit = TextEdit::singleline(&mut self.file_search_query)
                            .hint_text("Type to search...");
                        
                        // Automatically request focus when the modal is opened
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

        self.show_search_modal(ctx);
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
            self.handle_keyboard_shortcuts(ctx, ui);  // Keep existing keyboard shortcuts
            
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
        self.show_file_search_modal(ctx);
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