use eframe::egui;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::sync::Arc;
use tokio::runtime::Handle;
use lsp_types::{CompletionParams, TextDocumentPositionParams, Position, Url};
use crate::core::lsp_client::LspClient;

pub struct CodeEditor {
    pub code: String,
    pub current_file: Option<String>,
    syntax_set: Arc<SyntaxSet>,
    theme_set: Arc<ThemeSet>,
    current_syntax: String,
    lsp_client: Option<Arc<LspClient>>,
    runtime: Handle,
    completion_items: Vec<String>,
    hover_text: Option<String>,
    cursor_position: Position,
}

impl CodeEditor {
    pub fn new(runtime: Handle) -> Self {
        Self {
            code: String::new(),
            current_file: None,
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme_set: Arc::new(ThemeSet::load_defaults()),
            current_syntax: "JavaScript".to_string(),
            lsp_client: None,
            runtime,
            completion_items: Vec::new(),
            hover_text: None,
            cursor_position: Position::new(0, 0),
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
                }

                // Display completion items
                if !self.completion_items.is_empty() {
                    egui::Window::new("Completions")
                        .fixed_pos(ui.cursor().min)
                        .show(ui.ctx(), |ui| {
                            for item in &self.completion_items {
                                if ui.button(item).clicked() {
                                    // Insert the completion item at the cursor position
                                    let (start, end) = self.get_word_boundaries();
                                    self.code.replace_range(start..end, item);
                                }
                            }
                        });
                }

                // Display hover information
                if let Some(hover_text) = &self.hover_text {
                    egui::Window::new("Hover")
                        .fixed_pos(ui.cursor().min + egui::vec2(0.0, 20.0))
                        .show(ui.ctx(), |ui| {
                            ui.label(hover_text);
                        });
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
            self.runtime.spawn(async move {
                lsp_client.did_change(lsp_types::DidChangeTextDocumentParams {
                    text_document: lsp_types::VersionedTextDocumentIdentifier {
                        uri,
                        version: 0, // You might want to implement proper versioning
                    },
                    content_changes: vec![lsp_types::TextDocumentContentChangeEvent {
                        range: None,
                        range_length: None,
                        text: code,
                    }],
                }).await;
            });
        }
    }

    fn update_completion(&mut self) {
        if let (Some(lsp_client), Some(current_file)) = (&self.lsp_client, &self.current_file) {
            let lsp_client = Arc::clone(lsp_client);
            let position = self.cursor_position.clone();
            let uri = Url::from_file_path(current_file).expect("Failed to create URL from path");
            let runtime = self.runtime.clone();
            let completion_items = self.completion_items.clone();
            runtime.spawn(async move {
                if let Ok(Some(completion_response)) = lsp_client.completion(CompletionParams {
                    text_document_position: TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier { uri },
                        position,
                    },
                    context: None,
                    work_done_progress_params: Default::default(),
                    partial_result_params: Default::default(),
                }).await {
                    // Update completion items
                    if let Some(items) = completion_response.items {
                        let new_items: Vec<String> = items.into_iter()
                            .map(|item| item.label)
                            .collect();
                        *completion_items.lock().unwrap() = new_items;
                    }
                }
            });
        }
    }

    fn update_hover(&mut self) {
        if let (Some(lsp_client), Some(current_file)) = (&self.lsp_client, &self.current_file) {
            let lsp_client = Arc::clone(lsp_client);
            let position = self.cursor_position.clone();
            let uri = Url::from_file_path(current_file).expect("Failed to create URL from path");
            let runtime = self.runtime.clone();
            let hover_text = self.hover_text.clone();
            runtime.spawn(async move {
                if let Ok(Some(hover)) = lsp_client.hover(lsp_types::HoverParams {
                    text_document_position_params: TextDocumentPositionParams {
                        text_document: lsp_types::TextDocumentIdentifier { uri },
                        position,
                    },
                    work_done_progress_params: Default::default(),
                }).await {
                    // Update hover text
                    if let Some(contents) = hover.contents.as_markup_content() {
                        *hover_text.lock().unwrap() = Some(contents.value.clone());
                    }
                }
            });
        }
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
