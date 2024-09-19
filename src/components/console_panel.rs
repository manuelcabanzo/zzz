use eframe::egui;
use std::sync::{Arc, Mutex};
use crate::core::terminal::Terminal;

pub struct ConsolePanel {
    terminal: Arc<Mutex<Terminal>>,
}


impl ConsolePanel {
    pub fn new(terminal: Arc<Mutex<Terminal>>) -> Self {
        Self { terminal }
    }

    pub fn show(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("console_panel")
            .resizable(true)
            .default_height(250.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Console");
                    if ui.button("Clear").clicked() {
                        if let Ok(mut terminal) = self.terminal.lock() {
                            terminal.clear_output();
                        }
                    }
                });

                if let Ok(mut terminal) = self.terminal.lock() {
                    terminal.render(ui);
                }
            });
    }

    pub fn log(&self, message: &str) {
        if let Ok(mut terminal) = self.terminal.lock() {
            terminal.append_log(message);
        }
    }

    pub fn update(&self) {
        if let Ok(mut terminal) = self.terminal.lock() {
            terminal.update();
        }
    }
}
