use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
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
                    let output_tx_clone = output_tx.clone();
                    thread::spawn(move || {
                        execute_command(&input, output_tx_clone);
                    });
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

    pub fn append_log(&mut self, message: &str) {
        self.output.push_back(message.to_string());
        if self.output.len() > 100 {
            self.output.pop_front();
        }
        println!("{}", message);
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
    }
}

fn execute_command(command: &str, output_tx: Sender<String>) {
    let process = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(&["/C", command])
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    } else {
        Command::new("sh")
            .arg("-c")
            .arg(format!("stdbuf -oL -eL {}", command)) // Use stdbuf to disable buffering
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
    };

    if let Ok(mut process) = process {
        if let Some(stdout) = process.stdout.take() {
            let stdout_reader = BufReader::new(stdout);
            let output_tx_clone = output_tx.clone();
            thread::spawn(move || {
                for line in stdout_reader.lines() {
                    if let Ok(line) = line {
                        output_tx_clone.send(line).expect("Failed to send stdout line");
                    }
                }
            });
        }

        if let Some(stderr) = process.stderr.take() {
            let stderr_reader = BufReader::new(stderr);
            thread::spawn(move || {
                for line in stderr_reader.lines() {
                    if let Ok(line) = line {
                        output_tx.send(line).expect("Failed to send stderr line");
                    }
                }
            });
        }

        let _ = process.wait(); // Wait for the process to finish
    } else {
        let _ = output_tx.send("Failed to execute command".to_string());
    }
}
