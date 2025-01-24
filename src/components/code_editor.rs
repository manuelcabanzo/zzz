use eframe::egui;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::sync::Arc;
use std::path::Path;
use serde::{Deserialize, Serialize};


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
}

fn determine_syntax_from_path(path: &Path, syntax_set: &SyntaxSet) -> String {
    syntax_set
        .find_syntax_for_file(path)
        .ok()
        .flatten()
        .map(|syntax| syntax.name.clone())
        .unwrap_or_else(|| "Plain Text".to_string())
}

#[derive(Debug, Clone)]
pub struct Buffer {
    pub content: String,
    pub file_path: Option<String>,
    pub syntax: String,
    pub is_modified: bool,
    pub cursor_position: CursorPosition,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            content: String::new(),
            file_path: None,
            syntax: "Plain Text".to_string(),
            is_modified: false,
            cursor_position: CursorPosition { line: 0, column: 0 },
        }
    }

    pub fn from_file(content: String, file_path: String, syntax: String) -> Self {
        Self {
            content,
            file_path: Some(file_path),
            syntax,
            is_modified: false,
            cursor_position: CursorPosition { line: 0, column: 0 },
        }
    }

    pub fn set_cursor_position(&mut self, line: usize, column: usize) {
        self.cursor_position = CursorPosition { 
            line: line.saturating_sub(1), // Convert to 0-based index
            column 
        };
    }
}

pub struct CodeEditor {
    pub code: String,
    pub buffers: Vec<Buffer>,
    pub active_buffer_index: Option<usize>,
    pub current_file: Option<String>,
    pub search_highlight_text: Option<String>, // Add this field
    syntax_set: Arc<SyntaxSet>,
    theme_set: Arc<ThemeSet>,
}

impl CodeEditor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            buffers: Vec::new(),
            active_buffer_index: None,
            current_file: None,
            search_highlight_text: None, // Initialize
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme_set: Arc::new(ThemeSet::load_defaults()),
        }
    }

    pub fn create_new_buffer(&mut self) -> usize {
        let buffer = Buffer::new();
        self.buffers.push(buffer);
        let index = self.buffers.len() - 1;
        self.active_buffer_index = Some(index);
        index
    }

    pub fn open_file(&mut self, content: String, file_path: String) -> usize {
        // Check if the file is already open
        if let Some(index) = self.buffers.iter().position(|b| b.file_path.as_ref() == Some(&file_path)) {
            self.active_buffer_index = Some(index);
            return index;
        }

        let syntax = determine_syntax_from_path(Path::new(&file_path), &self.syntax_set);
        let buffer = Buffer::from_file(content, file_path, syntax);
        self.buffers.push(buffer);
        let index = self.buffers.len() - 1;
        self.active_buffer_index = Some(index);
        index
    }

    pub fn close_buffer(&mut self, index: usize) {
        if index < self.buffers.len() {
            self.buffers.remove(index);
            
            // Update active buffer index
            if let Some(active_index) = self.active_buffer_index {
                if active_index == index {
                    self.active_buffer_index = if self.buffers.is_empty() {
                        None
                    } else {
                        Some(active_index.saturating_sub(1))
                    };
                } else if active_index > index {
                    self.active_buffer_index = Some(active_index - 1);
                }
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, available_height: f32) {

        let mut buffer_to_close = None;

        ui.vertical(|ui| {
            // Header section with tabs
            ui.horizontal_wrapped(|ui| {
                for (index, buffer) in self.buffers.iter().enumerate() {
                    let is_active = Some(index) == self.active_buffer_index;
                    let file_name = buffer.file_path
                        .as_ref()
                        .and_then(|p| std::path::Path::new(p).file_name())
                        .and_then(|n| n.to_str())
                        .unwrap_or("untitled");
            
                    ui.horizontal(|ui| {
                        let mut text = egui::RichText::new(file_name);
                        if buffer.is_modified {
                            text = text.italics();
                        }
                        if is_active {
                            text = text.strong();
                        }
            
                        if ui.selectable_label(is_active, text).clicked() {
                            self.active_buffer_index = Some(index);
                        }
            
                        // Add the close button within the same horizontal layout
                        if ui.small_button("×").clicked() {
                            buffer_to_close = Some(index);
                        }
                    });
                }
            });

            if let Some(active_index) = self.active_buffer_index {
                if let Some(buffer) = self.buffers.get_mut(active_index) {
                    // Syntax selector
                    egui::ComboBox::from_label("Syntax")
                        .selected_text(&buffer.syntax)
                        .show_ui(ui, |ui| {
                            for syntax in self.syntax_set.syntaxes() {
                                ui.selectable_value(&mut buffer.syntax, syntax.name.clone(), &syntax.name);
                            }
                        });

                    // Calculate remaining height for the editor
                    let header_height = ui.min_rect().height();
                    let editor_height = available_height - header_height;
                    let search_highlight = self.search_highlight_text.clone();

                    // Code editing area with syntax highlighting
                    egui::ScrollArea::vertical()
                        .id_source(format!("buffer_{}_scroll_area", active_index))
                        .auto_shrink([false; 2])
                        .max_height(editor_height)
                        .show_viewport(ui, |ui, viewport| {
                            let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                                let mut layout_job = highlight_syntax(
                                    ui,
                                    string,
                                    &self.syntax_set,
                                    &self.theme_set,
                                    &buffer.syntax,
                                    search_highlight.as_deref() // Pass search highlight
                                );
                                layout_job.wrap.max_width = wrap_width;
                                ui.fonts(|f| f.layout_job(layout_job))
                            };

                            if viewport.intersects(ui.max_rect()) {
                                if ui.add_sized(
                                    [ui.available_width(), editor_height],
                                    egui::TextEdit::multiline(&mut buffer.content)
                                        .desired_width(f32::INFINITY)
                                        .font(egui::TextStyle::Monospace)
                                        .layouter(&mut layouter)
                                ).changed() {
                                    buffer.is_modified = true;
                                }
                            }
                        });
                }
            }
        });

        if let Some(index) = buffer_to_close {
            self.close_buffer(index);
        }
        
    }

    pub fn search(&mut self, search_term: &str) {
        // Set the search highlight text
        self.search_highlight_text = Some(search_term.to_string());

        // If an active buffer exists, find and set cursor to first occurrence
        if let Some(buffer) = self.get_active_buffer_mut() {
            let mut highlighted_content = buffer.content.clone();
            let mut end = 0;
            while let Some(next_position) = highlighted_content[end..].find(search_term) {
                end = end + next_position;
                highlighted_content.insert_str(end, "<mark>");
                end += "<mark>".len();
                highlighted_content.insert_str(end + search_term.len(), "</mark>");
                end += "</mark>".len();
                end += search_term.len();
            }
            buffer.content = highlighted_content;

            // Set cursor position to first occurrence
            if let Some(position) = buffer.content.find(search_term) {
                let (line, column) = calculate_line_column(&buffer.content, position);
                buffer.set_cursor_position(line, column);
            }
        }
    }

    pub fn get_active_buffer(&self) -> Option<&Buffer> {
        self.active_buffer_index.and_then(|i| self.buffers.get(i))
    }

    pub fn get_active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        self.active_buffer_index.and_then(|i| self.buffers.get_mut(i))
    }

    pub fn get_active_content(&self) -> String {
        self.get_active_buffer()
            .map(|buffer| buffer.content.clone())
            .unwrap_or_default()
    }
}

fn highlight_syntax(
    _ui: &egui::Ui,
    code: &str,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    current_syntax: &str,
    search_highlight: Option<&str>,
) -> egui::text::LayoutJob {
    let syntax = syntax_set.find_syntax_by_name(current_syntax)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, &theme_set.themes["base16-ocean.dark"]);

    let mut job = egui::text::LayoutJob::default();

    for line in LinesWithEndings::from(code) {
        let highlighted = highlighter.highlight_line(line, syntax_set).unwrap();
        
        if let Some(highlight_text) = search_highlight {
            if line.contains(highlight_text) {
                let highlight_format = egui::TextFormat {
                    background: egui::Color32::from_rgba_unmultiplied(255, 255, 0, 100),
                    ..egui::TextFormat::default()
                };
                
                let parts: Vec<&str> = line.split(highlight_text).collect();
                for (i, part) in parts.iter().enumerate() {
                    // Add the non-highlight part
                    for (style, text) in highlighter.highlight_line(part, syntax_set).unwrap() {
                        job.append(text, 0.0, style_to_text_format(style));
                    }
                    
                    // Add the highlight part if not the last segment
                    if i < parts.len() - 1 {
                        job.append(highlight_text, 0.0, highlight_format.clone()); // Add .clone()
                    }
                }
                continue;
            }
        }

        // Normal syntax highlighting if no search highlight
        for (style, text) in highlighted {
            job.append(text, 0.0, style_to_text_format(style));
        }
    }

    job
}

fn calculate_line_column(text: &str, position: usize) -> (usize, usize) {
    let lines = text[..position].split('\n');
    let line = lines.count();
    let last_line = text[..position].lines().last().unwrap_or("");
    let column = last_line.len() + 1;
    (line, column)
}

fn style_to_text_format(style: Style) -> egui::TextFormat {
    let color = egui::Color32::from_rgb(style.foreground.r, style.foreground.g, style.foreground.b);
    egui::TextFormat {
        color,
        ..egui::TextFormat::default()
    }
}