use eframe::egui;
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub struct CursorPosition {
    pub line: usize,
    pub column: usize,
}

pub struct CodeEditor {
    pub code: String,
    pub current_file: Option<String>,
    syntax_set: Arc<SyntaxSet>,
    theme_set: Arc<ThemeSet>,
    current_syntax: String,
}

impl CodeEditor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            current_file: None,
            syntax_set: Arc::new(SyntaxSet::load_defaults_newlines()),
            theme_set: Arc::new(ThemeSet::load_defaults()),
            current_syntax: "Java".to_string(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, available_height: f32) {
        ui.vertical(|ui| {
            // Header section with fixed height
            ui.heading("Code Editor");
            if let Some(file) = &self.current_file {
                ui.label(format!("Editing: {}", file));
            }
            
            egui::ComboBox::from_label("Syntax")
                .selected_text(&self.current_syntax)
                .show_ui(ui, |ui| {
                    for syntax in self.syntax_set.syntaxes() {
                        ui.selectable_value(&mut self.current_syntax, syntax.name.clone(), &syntax.name);
                    }
                });
    
            // Calculate remaining height for the editor
            let header_height = ui.min_rect().height();
            let editor_height = available_height - header_height;
    
            // Code editing area with syntax highlighting
            egui::ScrollArea::vertical()
                .id_source("code_editor_scroll_area")
                .auto_shrink([false; 2])
                .max_height(editor_height)
                .show_viewport(ui, |ui, viewport| {
                    let mut layouter = |ui: &egui::Ui, string: &str, wrap_width: f32| {
                        let mut layout_job = highlight_syntax(
                            ui, 
                            string, 
                            &self.syntax_set, 
                            &self.theme_set, 
                            &self.current_syntax
                        );
                        layout_job.wrap.max_width = wrap_width;
                        ui.fonts(|f| f.layout_job(layout_job))
                    };
    
                    if viewport.intersects(ui.max_rect()) {
                        ui.add_sized(
                            [ui.available_width(), editor_height],
                            egui::TextEdit::multiline(&mut self.code)
                                .desired_width(f32::INFINITY)
                                .font(egui::TextStyle::Monospace)
                                .layouter(&mut layouter)
                        );
                    }
                });
        });
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