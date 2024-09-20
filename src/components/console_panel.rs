use eframe::egui;
use std::sync::{Arc, Mutex};
use crossbeam_channel::Receiver;
use crate::core::terminal::Terminal;

pub struct ConsolePanel {
    terminal: Arc<Mutex<Terminal>>,
    output_receiver: Receiver<String>,
    output: Vec<String>,
    input: String,
}

impl ConsolePanel {
    pub fn new(terminal: Arc<Mutex<Terminal>>, output_receiver: Receiver<String>) -> Self {
        Self { 
            terminal,
            output_receiver,
            output: Vec::new(),
            input: String::new(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("console_panel")
            .resizable(false)
            .default_height(280.0)
            .max_height(280.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Console");
                    if ui.button("Clear").clicked() {
                        if let Ok(mut terminal) = self.terminal.lock() {
                            terminal.clear_output();
                        }
                        self.output.clear();
                    }
                });

                let available_height = ui.available_height();
                let log_height = available_height - 30.0; // Reserve space for input and heading

                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(log_height)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        for line in &self.output {
                            ui.label(line);
                        }
                    });

                ui.horizontal(|ui| {
                    ui.label(">");
                    let response = ui.add(egui::TextEdit::singleline(&mut self.input).desired_width(ui.available_width() - 20.0));
                    if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if let Ok(terminal) = self.terminal.lock() {
                            terminal.execute(self.input.clone());
                        }
                        self.input.clear();
                    }
                });
            });

        // Request a repaint to update the console more frequently
        ctx.request_repaint();
    }

    pub fn log(&mut self, message: &str) {
        if let Ok(mut terminal) = self.terminal.lock() {
            terminal.append_log(message);
        }
        self.output.push(message.to_string());
        if self.output.len() > 1000 {
            self.output.remove(0);
        }
    }

    pub fn update(&mut self) {
        while let Ok(message) = self.output_receiver.try_recv() {
            self.output.push(message);
            if self.output.len() > 1000 {
                self.output.remove(0);
            }
        }
    }
}
