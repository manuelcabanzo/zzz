use crate::core::terminal::Terminal;
use std::path::PathBuf;
use eframe::egui;

/// Represents the console panel in the IDE.
pub struct ConsolePanel {
    /// The terminal instance used for executing commands and displaying output.
    terminal: Terminal,
    
    /// The current project path associated with the console.
    pub project_path: Option<PathBuf>,
}

impl ConsolePanel {
    /// Creates a new `ConsolePanel` instance.
    /// 
    /// Initializes the terminal with the current working directory or defaults to `/` if unavailable.
    pub fn new() -> Self {
        let default_path = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/"));
        Self {
            terminal: Terminal::new(default_path),
            project_path: None,
        }
    }

    /// Sets the project path for the console panel and updates the terminal's working directory.
    /// 
    /// # Arguments
    /// - `path`: The new project path to set.
    pub fn set_project_path(&mut self, path: PathBuf) {
        self.project_path = Some(path.clone());
        self.terminal = Terminal::new(path);
    }

    /// Displays the terminal UI using `egui`.
    /// 
    /// # Arguments
    /// - `ui`: A mutable reference to the `egui` UI context.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        self.terminal.show(ui);
    }

    /// Updates the terminal state and handles keyboard shortcuts.
    /// 
    /// # Arguments
    /// - `ctx`: A reference to the `egui` application context.
    pub fn update(&mut self, ctx: &egui::Context) {
        self.terminal.update();
        self.terminal.handle_keyboard_shortcuts(ctx);
    }

    /// Logs a message to the terminal output.
    /// 
    /// # Arguments
    /// - `message`: The message to log.
    pub fn log(&mut self, message: &str) {
        self.terminal.add_output(message.to_string());
    }

    /// Sends an exit signal to the terminal.
    pub fn exit(&mut self) {
        self.terminal.exit();
    }

    /// Sends an interrupt signal to the terminal (e.g., Ctrl+C).
    pub fn send_interrupt(&mut self) {
        self.terminal.send_interrupt();
    }

    /// Sets the current working directory of the terminal.
    /// 
    /// # Arguments
    /// - `path`: The new working directory as a string.
    pub fn set_current_directory(&mut self, path: String) {
        let path_buf = PathBuf::from(path);
        *self.terminal.current_directory.lock().unwrap() = path_buf;
    }
}