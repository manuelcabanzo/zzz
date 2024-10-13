use eframe::egui;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::sync::{Arc, Mutex};
use lsp_types::{
    CompletionParams, 
    CompletionResponse, 
    TextDocumentPositionParams,
    VersionedTextDocumentIdentifier,
    DidChangeTextDocumentParams,
    TextDocumentContentChangeEvent,
    Position, 
    Url, 
    HoverContents, 
    HoverParams, 
    MarkedString
};
use tokio::runtime::Runtime;
use crate::core::lsp_client::LspClient;

pub struct CodeEditor {
    pub code: String,
    pub current_file: Option<String>,
    syntax_set: Arc<SyntaxSet>,
    runtime: Arc<Runtime>,
    theme_set: Arc<ThemeSet>,
    current_syntax: String,
    lsp_client: Option<Arc<LspClient>>,
    completion_items: Arc<Mutex<Vec<String>>>,
    hover_text: Arc<Mutex<Option<String>>>,   
    needs_update: Arc<Mutex<bool>>,
    cursor_position: Position,
    diagnostics: Arc<Mutex<Vec<(Position, String)>>>,
}

impl CodeEditor {
    pub fn new(runtime: Arc<Runtime>) -> Self {
        Self {
            code: String::new(),
            current_file: None,
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme_set: Arc::new(ThemeSet::load_defaults()),
            current_syntax: "JavaScript".to_string(),
            lsp_client: None,
            runtime,
            completion_items: Arc::new(Mutex::new(Vec::new())),
            hover_text: Arc::new(Mutex::new(None)),
            cursor_position: Position::new(0, 0),
            needs_update: Arc::new(Mutex::new(false)),
            diagnostics: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub fn set_lsp_client(&mut self, lsp_client: Arc<LspClient>) {
        self.lsp_client = Some(lsp_client);
    }

    pub fn show(&mut self, ui: &mut egui::Ui, available_height: f32) {
        ui.heading("Code Editor");
        if let Some(file) = &self.current_file {
            ui.label(format!("Editing: {}", file));
        }

        // Syntax selection dropdown
        egui::ComboBox::from_label("Syntax")
            .selected_text(&self.current_syntax)
            .show_ui(ui, |ui| {
                for syntax in self.syntax_set.syntaxes() {
                    ui.selectable_value(&mut self.current_syntax, syntax.name.clone(), &syntax.name);
                }
            });

        let syntax_set = Arc::clone(&self.syntax_set);
        let theme_set = Arc::clone(&self.theme_set);
        let current_syntax = self.current_syntax.clone();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                    let mut layout_job = highlight_syntax(ui, string, &syntax_set, &theme_set, &current_syntax);
                    layout_job.wrap.max_width = wrap_width;
                    ui.fonts(|f| f.layout_job(layout_job))
                };

                let text_edit = egui::TextEdit::multiline(&mut self.code)
                    .desired_width(f32::INFINITY)
                    .font(egui::TextStyle::Monospace)
                    .layouter(&mut layouter);

                let response = ui.add_sized([ui.available_width(), available_height], text_edit);

                if response.changed() {
                    self.update_lsp();
                }

                if let Some(cursor) = response.interact_pointer_pos() {
                    let row = cursor.y as u32;
                    let column = cursor.x as u32;
                    self.cursor_position = Position::new(row, column);
                    self.update_completion();
                    self.update_hover();
                    self.update_diagnostics(); // Add this line
                }

                if *self.needs_update.lock().unwrap() {
                    ui.ctx().request_repaint();
                    *self.needs_update.lock().unwrap() = false;
                }

                if response.changed() {
                    self.update_lsp();
                }

                // Display completion items
                
                let completion_items = self.completion_items.lock().unwrap().clone();
                if !completion_items.is_empty() {
                    egui::Window::new("Completions")
                        .fixed_pos(response.rect.left_bottom())
                        .show(ui.ctx(), |ui| {
                            for item in &completion_items {
                                if ui.button(item).clicked() {
                                    // Insert the completion item at the cursor position
                                    let (start, end) = self.get_word_boundaries();
                                    self.code.replace_range(start..end, item);
                                }
                            }
                        });
                }

                if let Some(lsp_client) = &self.lsp_client {
                    if let Some(current_file) = &self.current_file {
                        let uri = Url::from_file_path(current_file).expect("Failed to create URL from path");
                        let lsp_client_clone = Arc::clone(lsp_client);
                        let code = self.code.clone();
                        let runtime = Arc::clone(&self.runtime);

                        // Use spawn_blocking for potentially blocking operations
                        std::thread::spawn(move || {
                            runtime.block_on(async move {
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
                        });
                    }
                }

                // Display hover information
                if let Some(hover_text) = &*self.hover_text.lock().unwrap() {
                    egui::Window::new("Hover")
                        .fixed_pos(response.rect.left_top() + egui::vec2(0.0, -30.0))
                        .show(ui.ctx(), |ui| {
                            ui.label(hover_text);
                        });
                }

                // Display diagnostics
                let diagnostics = self.diagnostics.lock().unwrap().clone();
                for (position, message) in diagnostics {
                    let line_height = ui.text_style_height(&egui::TextStyle::Monospace);
                    let y_offset = position.line as f32 * line_height;
                    ui.painter().text(
                        response.rect.left_top() + egui::vec2(0.0, y_offset),
                        egui::Align2::LEFT_TOP,
                        &message,
                        egui::TextStyle::Monospace.resolve(ui.style()),
                        egui::Color32::RED,
                    );
                }     
            });
    }

    fn get_word_boundaries(&self) -> (usize, usize) {
        let line = self.code.lines().nth(self.cursor_position.line as usize).unwrap_or("");
        let start = line[..self.cursor_position.character as usize]
            .rfind(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + 1)
            .unwrap_or(0);
        let end = line[self.cursor_position.character as usize..]
            .find(|c: char| !c.is_alphanumeric() && c != '_')
            .map(|i| i + self.cursor_position.character as usize)
            .unwrap_or(line.len());
        (start, end)
    }

    fn update_lsp(&self) {
        if let (Some(lsp_client), Some(current_file)) = (&self.lsp_client, &self.current_file) {
            let lsp_client = Arc::clone(lsp_client);
            let code = self.code.clone();
            let uri = Url::from_file_path(current_file).expect("Failed to create URL from path");
            tokio::spawn(async move {
                lsp_client.did_change(DidChangeTextDocumentParams {
                    text_document: VersionedTextDocumentIdentifier {
                        uri,
                        version: 0,
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

    fn update_completion(&self) {
        if let (Some(lsp_client), Some(current_file)) = (&self.lsp_client, &self.current_file) {
            let lsp_client = Arc::clone(lsp_client);
            let position = self.cursor_position.clone();
            let uri = match Url::from_file_path(current_file) {
                Ok(uri) => uri,
                Err(_) => {
                    eprintln!("Failed to create URL from path: {}", current_file);
                    return;
                }
            };
            let completion_items = Arc::clone(&self.completion_items);
            let needs_update = Arc::clone(&self.needs_update);

            tokio::spawn(async move {
                let completion_params = CompletionParams {
                    text_document_position: TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier { uri },
                        position,
                    },
                    context: None,
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                };

                match lsp_client.completion(completion_params).await {
                    Ok(Some(completion_response)) => {
                        let new_items = match completion_response {
                            CompletionResponse::Array(items) => {
                                items.into_iter().map(|item| item.label).collect()
                            },
                            CompletionResponse::List(list) => {
                                list.items.into_iter().map(|item| item.label).collect()
                            },
                        };

                        if let Ok(mut items) = completion_items.lock() {
                            *items = new_items;
                            if let Ok(mut update) = needs_update.lock() {
                                *update = true;
                            }
                        }
                    },
                    Ok(None) => {
                        eprintln!("No completion items returned");
                    },
                    Err(e) => {
                        eprintln!("Error getting completions: {:?}", e);
                    }
                }
            });
        }
    }

    fn update_hover(&self) {
        if let (Some(lsp_client), Some(current_file)) = (&self.lsp_client, &self.current_file) {
            let lsp_client = Arc::clone(lsp_client);
            let position = self.cursor_position.clone();
            let uri = match Url::from_file_path(current_file) {
                Ok(uri) => uri,
                Err(_) => {
                    eprintln!("Failed to create URL from path: {}", current_file);
                    return;
                }
            };
            let hover_text = Arc::clone(&self.hover_text);
            let needs_update = Arc::clone(&self.needs_update);

            tokio::spawn(async move {
                let hover_params = HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier { uri },
                        position,
                    },
                    work_done_progress_params: Default::default(),
                };

                match lsp_client.hover(hover_params).await {
                    Ok(Some(hover)) => {
                        let new_text = match hover.contents {
                            HoverContents::Scalar(scalar) => marked_string_to_string(scalar),
                            HoverContents::Array(array) => array.into_iter()
                                .map(marked_string_to_string)
                                .collect::<Vec<_>>()
                                .join("\n"),
                            HoverContents::Markup(markup) => markup.value,
                        };

                        if let Ok(mut text) = hover_text.lock() {
                            *text = Some(new_text);
                            if let Ok(mut update) = needs_update.lock() {
                                *update = true;
                            }
                        }
                    },
                    Ok(None) => {
                        eprintln!("No hover information returned");
                    },
                    Err(e) => {
                        eprintln!("Error getting hover information: {:?}", e);
                    }
                }
            });
        }
    }
    
    fn update_diagnostics(&self) {
        if let Some(lsp_client) = &self.lsp_client {
            let lsp_client = Arc::clone(lsp_client);
            let diagnostics = Arc::clone(&self.diagnostics);
            let needs_update = Arc::clone(&self.needs_update);
            let current_file = self.current_file.clone();

            tokio::spawn(async move {
                if let Some(file) = current_file {
                    let uri = Url::from_file_path(file).unwrap();
                    if let Ok(new_diagnostics) = lsp_client.get_diagnostics(uri).await {
                        let mut diagnostics = diagnostics.lock().unwrap();
                        *diagnostics = new_diagnostics.into_iter()
                            .map(|d| (d.range.start, d.message))
                            .collect();
                        *needs_update.lock().unwrap() = true;
                    }
                }
            });
        }
    }
}

fn marked_string_to_string(marked: MarkedString) -> String {
    match marked {
        MarkedString::String(s) => s,
        MarkedString::LanguageString(lang_string) => format!("```{}\n{}\n```", lang_string.language, lang_string.value),
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
