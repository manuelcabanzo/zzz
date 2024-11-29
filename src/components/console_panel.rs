use crate::core::terminal::Terminal;
use std::path::PathBuf;

pub struct ConsolePanel {
    terminal: Terminal,
    pub project_path: Option<PathBuf>,
}

impl ConsolePanel {
    pub fn new() -> Self {
        let default_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        Self {
            terminal: Terminal::new(default_path),
            project_path: None,
        }
    }

    pub fn set_project_path(&mut self, path: PathBuf) {
        self.project_path = Some(path.clone());
        self.terminal = Terminal::new(path);
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.terminal.show(ui);
    }

    pub fn update(&mut self) {
        self.terminal.update();
    }

    pub fn log(&mut self, message: &str) {
        self.terminal.add_output(message.to_string());
    }

    pub fn exit(&mut self) {
        self.terminal.exit();
    }

    pub fn set_current_directory(&mut self, path: String) {
        let path_buf = PathBuf::from(path);
        *self.terminal.current_directory.lock().unwrap() = path_buf;
    }
}
