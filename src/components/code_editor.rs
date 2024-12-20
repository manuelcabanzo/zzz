use eframe::egui;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::sync::Arc;
use lsp_types::{CompletionItem, Diagnostic};

pub struct CodeEditor {
    pub code: String,
    pub current_file: Option<String>,
    syntax_set: Arc<SyntaxSet>,
    theme_set: Arc<ThemeSet>,
    current_syntax: String,
    pub lsp_completions: Vec<CompletionItem>,
    pub lsp_diagnostics: Vec<Diagnostic>,
    pub show_completions: bool,
    pub selected_completion_index: usize,
    cursor_position: usize,
    cursor_range: Option<egui::text::CCursor>,
    pub completions: Vec<String>,
    pub diagnostics: Vec<String>,
}

impl CodeEditor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            current_file: None,
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme_set: Arc::new(ThemeSet::load_defaults()),
            current_syntax: "Java".to_string(),
            lsp_completions: Vec::new(),
            lsp_diagnostics: Vec::new(),
            show_completions: false,
            selected_completion_index: 0,
            cursor_position: 0,
            cursor_range: None,
            completions: vec![],
            diagnostics: vec![],
        }
    }

    pub fn update_completions(&mut self, completions: Vec<String>) {
        self.completions = completions;
    }

    pub fn update_diagnostics(&mut self, diagnostics: Vec<String>) {
        self.diagnostics = diagnostics;
    }
    
    pub fn show_completions(&mut self, ui: &mut egui::Ui) {
        println!("show_completions called. Completions: {:?}", self.completions);
        
        if !self.completions.is_empty() {
            // Use a more robust UI rendering approach
            egui::Window::new("Completions")
                .collapsible(false)
                .show(ui.ctx(), |ui| {
                    ui.heading("Available Completions");
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        for completion in &self.completions {
                            if ui.button(completion).clicked() {
                                // Handle completion selection
                                let cursor_pos = self.cursor_position;
                                self.code.insert_str(cursor_pos, completion);
                                self.show_completions = false;
                            }
                        }
                    });
                });
        } else {
            println!("No completions to display");
        }
    }

    fn handle_completion_keyboard_events(&mut self, ctx: &egui::Context) -> bool {
        if !self.show_completions || self.lsp_completions.is_empty() {
            return false;
        }

        let mut event_handled = false;
        
        if ctx.input(|i| i.key_pressed(egui::Key::Tab)) {
            self.selected_completion_index = (self.selected_completion_index + 1) % self.lsp_completions.len();
            event_handled = true;
        }
        
        if ctx.input(|i| i.key_pressed(egui::Key::Enter)) {
            self.apply_selected_completion();
            event_handled = true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.show_completions = false;
            event_handled = true;
        }

        event_handled
    }

    pub fn handle_completions(&mut self, ui: &mut egui::Ui) {
        if !self.show_completions || self.lsp_completions.is_empty() {
            return;
        }

        // Handle keyboard events first
        let event_handled = self.handle_completion_keyboard_events(ui.ctx());
        if event_handled {
            ui.ctx().memory_mut(|mem| mem.request_focus(ui.id()));
            return;
        }

        let menu_pos = self.get_menu_position(ui);
        
        let mut selected_idx = None;
        
        egui::Window::new("completions")
            .fixed_pos(menu_pos)
            .collapsible(false)
            .resizable(false)
            .title_bar(false)
            .show(ui.ctx(), |ui| {
                ui.set_min_width(200.0);
                egui::Frame::popup(ui.style()).show(ui, |ui| {
                    egui::ScrollArea::vertical()
                        .max_height(200.0)
                        .show(ui, |ui| {
                            for (index, completion) in self.lsp_completions.iter().enumerate() {
                                let is_selected = index == self.selected_completion_index;
                                let response = ui.selectable_label(
                                    is_selected,
                                    format!("{} {}", 
                                        match completion.kind {
                                            Some(lsp_types::CompletionItemKind::FUNCTION) => "ƒ",
                                            Some(lsp_types::CompletionItemKind::METHOD) => "○",
                                            Some(lsp_types::CompletionItemKind::VARIABLE) => "□",
                                            Some(lsp_types::CompletionItemKind::CLASS) => "◇",
                                            Some(lsp_types::CompletionItemKind::INTERFACE) => "◆",
                                            _ => "•",
                                        },
                                        &completion.label
                                    )
                                );

                                if response.clicked() {
                                    selected_idx = Some(index);
                                }
                            }
                        });
                });
            });

        // Handle selection after the window closes
        if let Some(index) = selected_idx {
            self.selected_completion_index = index;
            self.apply_selected_completion();
        }
    }

    fn get_menu_position(&self, ui: &egui::Ui) -> egui::Pos2 {
        let text_edit_rect = ui.min_rect();
        let text = self.code.clone();
        
        let font_id = ui.style().text_styles.get(&egui::TextStyle::Monospace).unwrap().clone();
        let font_id_clone = font_id.clone();
        let text_layout = ui.fonts(|f| {
            f.layout_job(egui::text::LayoutJob::simple(
                text,
                font_id,
                ui.visuals().text_color(),
                f32::INFINITY,
            ))
        });

        if let Some(cursor_range) = &self.cursor_range {
            let cursor = text_layout.from_ccursor(*cursor_range);
            let pos = text_layout.pos_from_cursor(&cursor);
            
            text_edit_rect.min + egui::Vec2::new(pos.min.x, pos.min.y) + 
                egui::vec2(0.0, ui.fonts(|f| f.row_height(&font_id_clone)))
        } else {
            text_edit_rect.min
        }
    }

    fn apply_selected_completion(&mut self) {
        if self.selected_completion_index >= self.lsp_completions.len() {
            // Safely handle out of bounds index
            self.show_completions = false;
            self.selected_completion_index = 0;
            return;
        }

        let selected_completion = &self.lsp_completions[self.selected_completion_index];
        
        // Get the current cursor position and text to determine insertion point
        let current_pos = self.cursor_position;
        let current_line = self.code[..current_pos]
            .lines()
            .last()
            .unwrap_or("")
            .to_string();

        // Determine the start position for the completion
        let start_pos = current_line
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);

        // Calculate positions for text replacement
        let replacement_start = current_pos - (current_line.len() - start_pos);
        let replacement_end = current_pos;

        // Get completion text, preferring insert_text over label if available
        let completion_text = selected_completion
            .insert_text
            .clone()
            .unwrap_or_else(|| selected_completion.label.clone());

        // Safely perform the text replacement
        if replacement_start <= replacement_end && replacement_end <= self.code.len() {
            let before = &self.code[..replacement_start];
            let after = &self.code[replacement_end..];
            self.code = format!("{}{}{}", before, completion_text, after);
            
            // Update cursor position
            self.cursor_position = replacement_start + completion_text.len();
        }

        // Reset completion state
        self.show_completions = false;
        self.selected_completion_index = 0;
    }

    pub fn show(&mut self, ui: &mut egui::Ui, _available_height: f32) {
        ui.heading("Code Editor");
        if let Some(file) = &self.current_file {
            ui.label(format!("Editing: {}", file));
        }
        
        if self.show_completions {
            let completion_id = ui.make_persistent_id("completion_popup");
            ui.ctx().memory_mut(|mem| mem.request_focus(completion_id));
        }

        egui::ComboBox::from_label("Syntax")
            .selected_text(&self.current_syntax)
            .show_ui(ui, |ui| {
                for syntax in self.syntax_set.syntaxes() {
                    ui.selectable_value(&mut self.current_syntax, syntax.name.clone(), &syntax.name);
                }
            });

        // Error/Diagnostic Display
        if !self.lsp_diagnostics.is_empty() {
            ui.separator();
            ui.heading("Errors & Warnings");
            egui::ScrollArea::vertical()
                .id_source("lsp_diagnostics_scroll_area") // Assign a unique ID to the ScrollArea
                .max_height(100.0)
                .show(ui, |ui| {
                    for diagnostic in &self.lsp_diagnostics {
                        let severity = match diagnostic.severity {
                            Some(sev) => match sev {
                                lsp_types::DiagnosticSeverity::ERROR => "Error",
                                lsp_types::DiagnosticSeverity::WARNING => "Warning",
                                lsp_types::DiagnosticSeverity::INFORMATION => "Info",
                                lsp_types::DiagnosticSeverity::HINT => "Hint",
                                _ => "Unknown"
                            },
                            None => "Unknown"
                        };

                        ui.colored_label(
                            match severity {
                                "Error" => egui::Color32::RED,
                                "Warning" => egui::Color32::YELLOW,
                                _ => egui::Color32::GRAY
                            },
                            format!("{}: {}", severity, diagnostic.message)
                        );
                    }
                });
        }

        // egui::ScrollArea::vertical()
        //     .max_height(height)
        //     .show(ui, |ui| {
        //         ui.text_edit_multiline(&mut self.code);

        //         if !self.completions.is_empty() {
        //             for completion in &self.completions {
        //                 ui.label(format!("Completion: {}", completion));
        //             }
        //         }

        //         if !self.diagnostics.is_empty() {
        //             for diagnostic in &self.diagnostics {
        //                 ui.colored_label(egui::Color32::RED, format!("Error: {}", diagnostic));
        //             }
        //         }
        //     });
            
        // Code editing area with syntax highlighting
        let syntax_set = Arc::clone(&self.syntax_set);
        let theme_set = Arc::clone(&self.theme_set);
        let current_syntax = self.current_syntax.clone();

        egui::ScrollArea::vertical()
            .id_source("code_editor_scroll_area") // Assign a unique ID to the ScrollArea
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                    let mut layout_job = highlight_syntax(ui, string, &syntax_set, &theme_set, &current_syntax);
                    layout_job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(layout_job))
                };

                let text_edit_response = egui::TextEdit::multiline(&mut self.code)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .layouter(&mut layouter)
                    .show(ui); // Show the TextEdit widget directly

                if let Some(cursor_range) = text_edit_response.cursor_range {
                    self.cursor_position = cursor_range.primary.ccursor.index;
                    self.cursor_range = Some(cursor_range.primary.ccursor);
                }
            });

            self.handle_completions(ui);

    }
}

fn highlight_syntax(
    _ui: &egui::Ui,
    code: &str,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    current_syntax: &str,
) -> egui::text::LayoutJob {
    let syntax = syntax_set.find_syntax_by_name(current_syntax)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, &theme_set.themes["base16-ocean.dark"]);

    let mut job = egui::text::LayoutJob::default();

    for line in LinesWithEndings::from(code) {
        let highlighted = highlighter.highlight_line(line, syntax_set).unwrap();
        for (style, text) in highlighted {
            job.append(text, 0.0, style_to_text_format(style));
        }
    }

    job
}

fn style_to_text_format(style: Style) -> egui::TextFormat {
    let color = egui::Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    egui::TextFormat {
        color,
        ..egui::TextFormat::default()
    }
}