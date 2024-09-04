use std::collections::VecDeque;
use std::process::Command;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use eframe::egui;

pub struct Terminal {
    input: String,
    output: VecDeque<String>,
    input_tx: Sender<String>,
    output_rx: Receiver<String>,
}

impl Terminal {
    pub fn new() -> Self {
        let (input_tx, input_rx) = channel::<String>();
        let (output_tx, output_rx) = channel::<String>();

        thread::spawn(move || {
            loop {
                if let Ok(input) = input_rx.recv() {
                    let output = execute_command(&input);
                    output_tx.send(output).expect("Failed to send output");
                }
            }
        });

        Self {
            input: String::new(),
            output: VecDeque::new(),
            input_tx,
            output_rx,
        }
    }

    pub fn update(&mut self) {
        while let Ok(output) = self.output_rx.try_recv() {
            self.output.push_back(output);
            if self.output.len() > 100 {
                self.output.pop_front();
            }
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                for line in &self.output {
                    ui.label(line);
                }
            });

            ui.horizontal(|ui| {
                ui.label(">");
                let response = ui.text_edit_singleline(&mut self.input);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.input_tx.send(self.input.clone()).expect("Failed to send input");
                    self.input.clear();
                }
            });
        });
    }
}

fn execute_command(command: &str) -> String {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", command])
            .output()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(command)
            .output()
    };

    match output {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
            let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
            format!("{}\n{}", stdout, stderr)
        }
        Err(e) => format!("Failed to execute command: {}", e),
    }
}
