use eframe::egui;
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

    pub fn show(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("console_panel")
            .resizable(true)
            .default_height(250.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Console");
                    if ui.button("Clear").clicked() {
                        self.terminal.clear_output();
                    }
                });

                self.terminal.render(ui);
            });
    }

    pub fn log(&mut self, message: &str) {
        self.terminal.append_log(message);
    }

    pub fn update(&mut self) {
        self.terminal.update();
    }
}
