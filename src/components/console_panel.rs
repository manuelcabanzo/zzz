use eframe::egui;
use std::sync::Arc;
use crossbeam_channel::Receiver;

pub struct ConsolePanel {
    output: Vec<String>,
    output_receiver: Receiver<String>,
    current_directory: Arc<std::sync::Mutex<String>>,
}

impl ConsolePanel {
    pub fn new(output_receiver: Receiver<String>) -> Self {
        Self {
            output: Vec::new(),
            output_receiver,
            current_directory: Arc::new(std::sync::Mutex::new(String::from("/")))
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("Console");

            let current_dir = self.current_directory.lock().unwrap().clone();
            ui.label(format!("Current Directory: {}", current_dir));

            let available_height = ui.available_height();
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(available_height - 40.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    for line in &self.output {
                        ui.label(line);
                    }
                });
        });
    }

    pub fn update(&mut self) {
        while let Ok(message) = self.output_receiver.try_recv() {
            self.output.push(message);
            if self.output.len() > 1000 {
                self.output.remove(0);
            }
        }
    }

    pub fn log(&mut self, message: &str) {
        self.output.push(message.to_string());
        if self.output.len() > 1000 {
            self.output.remove(0);
        }
    }

    pub fn set_current_directory(&self, path: String) {
        let mut current_dir = self.current_directory.lock().unwrap();
        *current_dir = path;
    }
}
