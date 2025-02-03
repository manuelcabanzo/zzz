use eframe::egui;
use image::GenericImageView;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::sync::Arc;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use serde::{Deserialize, Serialize};
use lru::LruCache;
use std::num::NonZeroUsize;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
}

struct HighlightCache {
    jobs: LruCache<(String, String), egui::text::LayoutJob>,
}

impl Default for HighlightCache {
    fn default() -> Self {
        Self::new()
    }
}

impl HighlightCache {
    fn new() -> Self {
        Self {
            jobs: LruCache::new(NonZeroUsize::new(100).unwrap()),
        }
    }
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
    pub search_highlight_text: Option<String>,
    pub search_highlight_expires_at: Option<Instant>,
    syntax_set: Arc<SyntaxSet>,
    theme_set: Arc<ThemeSet>,
    pub search_selected_line: Option<usize>,
    pub logo_texture: Option<egui::TextureHandle>,
    highlight_cache: HighlightCache,
}

impl CodeEditor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            buffers: Vec::new(),
            active_buffer_index: None,
            current_file: None,
            search_highlight_text: None,
            search_highlight_expires_at: None,
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme_set: Arc::new(ThemeSet::load_defaults()),
            search_selected_line: None,
            logo_texture: None,
            highlight_cache: HighlightCache::new(),
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

    pub fn load_logo(&mut self, ctx: &egui::Context) -> Result<(), image::ImageError> {
        if self.logo_texture.is_none() {
            let logo_path = PathBuf::from("src/resources/blacksquare.png");
            let img = image::open(&logo_path)?;
            let dimensions = img.dimensions();
            let rgba = img.into_rgba8();
            let pixels = rgba.as_flat_samples();
            let image = egui::ColorImage::from_rgba_unmultiplied(
                [dimensions.0 as _, dimensions.1 as _],
                pixels.as_slice(),
            );
            self.logo_texture = Some(ctx.load_texture(
                "logo",
                image,
                egui::TextureOptions::default(),
            ));
        }
        Ok(())
    }

    pub fn close_buffer(&mut self, index: usize) {
        if index < self.buffers.len() {
            self.buffers.remove(index);
            
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
        self.clear_expired_highlights();
        let mut buffer_to_close = None;

        ui.vertical(|ui| {
            self.show_tabs(ui, &mut buffer_to_close);

            if self.buffers.is_empty() {
                self.show_welcome_screen(ui, available_height);
            } else {
                self.show_active_buffer(ui, available_height);
            }
        });

        if let Some(index) = buffer_to_close {
            self.close_buffer(index);
        }
    }

    fn show_tabs(&mut self, ui: &mut egui::Ui, buffer_to_close: &mut Option<usize>) {
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
        
                    if ui.small_button("×").clicked() {
                        *buffer_to_close = Some(index);
                    }
                });
            }
        });
    }

    fn show_welcome_screen(&self, ui: &mut egui::Ui, available_height: f32) {
        let available_rect = ui.available_rect_before_wrap();
        
        egui::Frame::none()
            .fill(ui.style().visuals.window_fill)
            .show(ui, |ui| {
                let logo_height = 128.0;
                let heading_height = 30.0;
                let shortcuts_height = 7.0 * 20.0;
                let spacing = 20.0 * 3.0;
                let total_content_height = logo_height + heading_height + shortcuts_height + spacing;
                
                let vertical_margin = (available_height - total_content_height) / 2.0;
                
                ui.allocate_ui_with_layout(
                    available_rect.size(),
                    egui::Layout::centered_and_justified(egui::Direction::TopDown),
                    |ui| {
                        ui.add_space(vertical_margin.max(0.0));
                        
                        ui.vertical_centered(|ui| {
                            if let Some(logo) = &self.logo_texture {
                                ui.image(logo);
                                ui.add_space(20.0);
                            }
                            
                            ui.heading("Welcome to ZZZ IDE");
                            ui.add_space(20.0);
                            
                            ui.label("Shortcuts:");
                            ui.label("Ctrl+O: Open folder");
                            ui.label("Ctrl+P: Search files");
                            ui.label("Ctrl+F: Find in current file");
                            ui.label("Ctrl+Shift+F: Find in project");
                            ui.label("Ctrl+M: Open settings");
                            ui.label("Ctrl+S: Save current file");
                            ui.add_space(20.0);
                            
                            ui.label("Start by opening a folder or creating a new file");
                        });
                        
                        ui.add_space(vertical_margin.max(0.0));
                    },
                );
            });
    }

    fn show_active_buffer(&mut self, ui: &mut egui::Ui, available_height: f32) {
        if let Some(active_index) = self.active_buffer_index {
            if let Some(buffer) = self.buffers.get_mut(active_index) {
                let syntax = buffer.syntax.clone();
                
                // Syntax selector
                let syntax_set = &self.syntax_set;
                egui::ComboBox::from_label("Syntax")
                    .selected_text(&syntax)
                    .show_ui(ui, |ui| {
                        for syntax_def in syntax_set.syntaxes() {
                            ui.selectable_value(&mut buffer.syntax, syntax_def.name.clone(), &syntax_def.name);
                        }
                    });
    
                let header_height = ui.min_rect().height();
                let editor_height = available_height - header_height;
                let search_highlight = self.search_highlight_text.clone();
                let selected_line = self.search_selected_line;
    
                // Create a persistent ScrollArea
                egui::ScrollArea::vertical()
                    .id_source(format!("buffer_{}_scroll_area", active_index))
                    .auto_shrink([false; 2])
                    .max_height(editor_height)
                    .show(ui, |ui| {  // Changed from show_viewport to show
                        let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                            let mut layout_job = highlight_syntax(
                                string,
                                &self.syntax_set,
                                &self.theme_set,
                                &buffer.syntax,
                                search_highlight.as_deref(),
                                selected_line,
                                &mut self.highlight_cache,
                            );
                            layout_job.wrap.max_width = wrap_width;
                            ui.fonts(|f| f.layout_job(layout_job))
                        };

                        // Remove the viewport intersection check
                        if ui.add_sized(
                            [ui.available_width(), ui.available_height()],  // Use available_height instead of fixed editor_height
                            egui::TextEdit::multiline(&mut buffer.content)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                                .layouter(&mut layouter)
                        ).changed() {
                            buffer.is_modified = true;
                        }
                    });
            }
        }
    }

    pub fn search(&mut self, search_term: &str, selected_line_number: Option<usize>) {
        self.search_highlight_text = Some(search_term.to_string());
        self.search_highlight_expires_at = Some(Instant::now() + Duration::from_secs_f64(0.5));
        self.search_selected_line = selected_line_number;

        if let Some(buffer) = self.get_active_buffer_mut() {
            if let Some(position) = buffer.content.find(search_term) {
                let (line, column) = calculate_line_column(&buffer.content, position);
                buffer.set_cursor_position(line, column);
            }
        }
    }

    pub fn clear_expired_highlights(&mut self) {
        if let Some(expires_at) = self.search_highlight_expires_at {
            if Instant::now() >= expires_at {
                self.search_highlight_text = None;
                self.search_highlight_expires_at = None;
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
    code: &str,
    syntax_set: &SyntaxSet,
    theme_set: &ThemeSet,
    current_syntax: &str,
    search_highlight: Option<&str>,
    selected_line: Option<usize>,
    cache: &mut HighlightCache,
) -> egui::text::LayoutJob {
    let syntax = syntax_set.find_syntax_by_name(current_syntax)
        .unwrap_or_else(|| syntax_set.find_syntax_plain_text());
    let mut highlighter = HighlightLines::new(syntax, &theme_set.themes["base16-ocean.dark"]);

    let mut job = egui::text::LayoutJob::default();
    
    // Don't use cache when we have active highlighting
    if search_highlight.is_none() && selected_line.is_none() {
        let cache_key = (current_syntax.to_string(), code.to_string());
        if let Some(cached_job) = cache.jobs.get(&cache_key) {
            return cached_job.clone();
        }
    }

    for (line_index, line) in LinesWithEndings::from(code).enumerate() {
        let is_selected_line = selected_line.map_or(false, |sel| line_index + 1 == sel);
        
        // Apply background highlight for selected line
        if is_selected_line {
            job.append(
                "",
                0.0,
                egui::TextFormat {
                    background: egui::Color32::from_rgba_unmultiplied(60, 60, 60, 255),
                    ..Default::default()
                },
            );
        }

        if let Some(search_text) = search_highlight {
            let mut last_end = 0;
            for (start, end) in find_all_occurrences(line, search_text) {
                // Add non-highlighted text before match
                if start > last_end {
                    for (style, text) in highlighter.highlight_line(&line[last_end..start], syntax_set).unwrap() {
                        job.append(text, 0.0, style_to_text_format(style));
                    }
                }

                // Add highlighted text
                let highlight_format = egui::TextFormat {
                    background: egui::Color32::from_rgba_unmultiplied(255, 215, 0, 100), // golden highlight
                    ..Default::default()
                };
                job.append(&line[start..end], 0.0, highlight_format);

                last_end = end;
            }

            // Add remaining non-highlighted text
            if last_end < line.len() {
                for (style, text) in highlighter.highlight_line(&line[last_end..], syntax_set).unwrap() {
                    job.append(text, 0.0, style_to_text_format(style));
                }
            }
        } else {
            // No search highlight, just apply syntax highlighting
            for (style, text) in highlighter.highlight_line(line, syntax_set).unwrap() {
                job.append(text, 0.0, style_to_text_format(style));
            }
        }
    }

    // Only cache when there's no active highlighting
    if search_highlight.is_none() && selected_line.is_none() {
        let cache_key = (current_syntax.to_string(), code.to_string());
        cache.jobs.put(cache_key, job.clone());
    }

    job
}

fn find_all_occurrences(text: &str, pattern: &str) -> Vec<(usize, usize)> {
    let mut results = Vec::new();
    let mut start = 0;
    while let Some(pos) = text[start..].find(pattern) {
        let absolute_pos = start + pos;
        results.push((absolute_pos, absolute_pos + pattern.len()));
        start = absolute_pos + 1;
    }
    results
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