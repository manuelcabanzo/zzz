use eframe::egui;
use std::sync::{Arc, Mutex};
use crossbeam_channel::Receiver;
use crate::core::terminal::Terminal;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ConsolePanel {
    terminal: Arc<Mutex<Terminal>>,
    output_receiver: Receiver<String>,
    output: Vec<String>,
    input: String,
    is_process_running: Arc<AtomicBool>,
}

impl ConsolePanel {
    pub fn new(terminal: Arc<Mutex<Terminal>>, output_receiver: Receiver<String>) -> Self {
        let is_process_running = Arc::new(AtomicBool::new(false));
        
        Self { 
            terminal,
            output_receiver,
            output: Vec::new(),
            input: String::new(),
            is_process_running,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.heading("Console");
                if ui.button("Clear").clicked() {
                    self.clear_console();
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
                let cwd = if let Ok(terminal) = self.terminal.lock() {
                    terminal.get_working_directory().to_string_lossy().into_owned()
                } else {
                    String::from("/")
                };
                ui.label(format!("{}> ", cwd));
                let response = ui.add(egui::TextEdit::singleline(&mut self.input).desired_width(ui.available_width() - 20.0));
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.execute_command();
                }
            });

            // Handle Ctrl+C
            if ui.input(|i| i.modifiers.ctrl && i.key_pressed(egui::Key::C)) {
                self.handle_ctrl_c();
            }
        });
    }

    fn execute_command(&mut self) {
        if let Ok(terminal) = self.terminal.lock() {
            terminal.execute(self.input.clone());
        }
        self.input.clear();
    }

    fn handle_ctrl_c(&mut self) {
        if self.is_process_running.load(Ordering::SeqCst) {
            if let Ok(terminal) = self.terminal.lock() {
                terminal.send_ctrl_c();
            }
            self.is_process_running.store(false, Ordering::SeqCst);
            self.output.push("^C".to_string());
        }
    }

    fn clear_console(&mut self) {
        if let Ok(mut terminal) = self.terminal.lock() {
            terminal.clear_output();
        }
        self.output.clear();
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

    pub fn set_working_directory(&mut self, path: PathBuf) {
        let mut terminal = self.terminal.lock().unwrap();
        terminal.set_working_directory(path);
    }
}
