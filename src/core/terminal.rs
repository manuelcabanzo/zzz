use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub struct Terminal {
    input: String,
    output: VecDeque<String>,
    input_tx: Sender<String>,
    output_rx: Receiver<String>,
    working_directory: Arc<Mutex<Option<PathBuf>>>,
}

impl Terminal {
    pub fn new() -> Self {
        let (input_tx, input_rx) = channel::<String>();
        let (output_tx, output_rx) = channel::<String>();
        let working_directory = Arc::new(Mutex::new(None));

        let working_directory_clone = Arc::clone(&working_directory);
        thread::spawn(move || {
            loop {
                if let Ok(input) = input_rx.recv() {
                    let output_tx_clone = output_tx.clone();
                    let working_dir = working_directory_clone.clone();
                    thread::spawn(move || {
                        let dir = working_dir.lock().unwrap().clone();
                        execute_command(&input, output_tx_clone, dir.as_ref());
                    });
                }
            }
        });

        Self {
            input: String::new(),
            output: VecDeque::new(),
            input_tx,
            output_rx,
            working_directory,
        }
    }

    pub fn set_working_directory(&self, path: PathBuf) {
        let mut working_dir = self.working_directory.lock().unwrap();
        *working_dir = Some(path);
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

    pub fn execute(&self, command: String) {
        let working_dir = self.working_directory.clone();
        let input_tx = self.input_tx.clone();
        thread::spawn(move || {
            let dir = working_dir.lock().unwrap().clone();
            execute_command(&command, input_tx, dir.as_ref());
        });
    }
}

fn execute_command(command: &str, output_tx: Sender<String>, working_dir: Option<&PathBuf>) {
    let mut process_command = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    };

    if let Some(dir) = working_dir {
        process_command.current_dir(dir);
    }

    let process = process_command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

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
