use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style, Theme};
use syntect::parsing::SyntaxSet;
use syntect::util::LinesWithEndings;

pub struct SyntaxHighlighter {
    syntax_set: SyntaxSet,
    theme: Theme,
}

impl SyntaxHighlighter {
    pub fn new() -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme_set = ThemeSet::load_defaults();
        
        // Use a high-contrast theme
        let theme = theme_set.themes["base16-mocha.dark"].clone();
        
        Self {
            syntax_set,
            theme,
        }
    }

    pub fn highlight(&self, code: &str, file_extension: &str) -> Vec<(Style, String)> {
        let syntax = match file_extension {
            "js" | "jsx" => self.syntax_set.find_syntax_by_extension("js")
                .or_else(|| self.syntax_set.find_syntax_by_name("JavaScript (Babel)"))
                .or_else(|| self.syntax_set.find_syntax_by_name("JavaScript")),
            "ts" => self.syntax_set.find_syntax_by_extension("ts")
                .or_else(|| self.syntax_set.find_syntax_by_name("TypeScript")),
            "tsx" => self.syntax_set.find_syntax_by_extension("tsx")
                .or_else(|| self.syntax_set.find_syntax_by_name("TSX"))
                .or_else(|| self.syntax_set.find_syntax_by_name("TypeScript"))
                .or_else(|| self.syntax_set.find_syntax_by_name("JavaScript (Babel)")),
            "rs" => self.syntax_set.find_syntax_by_extension("rs")
                .or_else(|| self.syntax_set.find_syntax_by_name("Rust")),
            "py" => self.syntax_set.find_syntax_by_extension("py")
                .or_else(|| self.syntax_set.find_syntax_by_name("Python")),
            "html" => self.syntax_set.find_syntax_by_extension("html")
                .or_else(|| self.syntax_set.find_syntax_by_name("HTML")),
            "css" => self.syntax_set.find_syntax_by_extension("css")
                .or_else(|| self.syntax_set.find_syntax_by_name("CSS")),
            _ => self.syntax_set.find_syntax_by_extension(file_extension),
        }.unwrap_or_else(|| self.syntax_set.find_syntax_plain_text());

        println!("Using syntax: {:?}", syntax.name);

        let mut h = HighlightLines::new(syntax, &self.theme);
        
        let result = LinesWithEndings::from(code)
            .flat_map(|line| {
                match h.highlight_line(line, &self.syntax_set) {
                    Ok(highlighted) => {
                        println!("Highlighted line: {:?}", highlighted);
                        highlighted
                            .into_iter()
                            .map(|(style, text)| (style, text.to_string()))
                            .collect::<Vec<_>>()
                    },
                    Err(e) => {
                        println!("Error highlighting line: {:?}", e);
                        vec![(Style::default(), line.to_string())]
                    },
                }
            })
            .collect();

        println!("Highlight result: {:?}", result);
        result
    }
}
