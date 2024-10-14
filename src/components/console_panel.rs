use crate::core::terminal::Terminal;

pub struct ConsolePanel {
    terminal: Terminal,
}

impl ConsolePanel {
    pub fn new() -> Self {
        Self {
            terminal: Terminal::new(),
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.terminal.show(ui);
    }

    pub fn update(&mut self) {
        self.terminal.update();
    }

    pub fn log(&mut self, message: &str) {
        // We'll keep this method for backwards compatibility
        self.terminal.output.push(message.to_string());
    }

    pub fn set_current_directory(&self, path: String) {
        *self.terminal.current_directory.lock().unwrap() = path;
    }
}
