use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::thread;
use eframe::egui;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use crossbeam_channel::{unbounded, Sender, Receiver};

pub struct Terminal {
    input: String,
    output: VecDeque<String>,
    input_tx: Sender<String>,
    output_rx: Receiver<String>,
    working_directory: Arc<Mutex<Option<PathBuf>>>,
    output_tx: Sender<String>,
}

impl Terminal {
    pub fn new() -> (Self, Receiver<String>) {
        let (input_tx, input_rx) = unbounded::<String>();
        let (output_tx, output_rx) = unbounded::<String>();
        let working_directory = Arc::new(Mutex::new(None));

        let working_directory_clone = Arc::clone(&working_directory);
        let output_tx_clone = output_tx.clone();
        thread::spawn(move || {
            loop {
                if let Ok(input) = input_rx.recv() {
                    let output_tx = output_tx_clone.clone();
                    let working_dir = working_directory_clone.clone();
                    thread::spawn(move || {
                        let dir = working_dir.lock().unwrap().clone();
                        execute_command(&input, output_tx, dir);
                    });
                }
            }
        });

        (Self {
            input: String::new(),
            output: VecDeque::new(),
            input_tx,
            output_rx: output_rx.clone(),
            working_directory,
            output_tx,
        }, output_rx)
    }

    pub fn set_working_directory(&self, path: PathBuf) {
        let mut working_dir = self.working_directory.lock().unwrap();
        *working_dir = Some(path.clone());
        // Log the new working directory
        self.output_tx.send(format!("Working directory set to: {:?}", path)).expect("Failed to send log message");
    }

    pub fn update(&mut self) {
        while let Ok(output) = self.output_rx.try_recv() {
            self.output.push_back(output);
            if self.output.len() > 1000 {
                self.output.pop_front();
            }
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .max_height(280.0)
                .show(ui, |ui| {
                    for line in &self.output {
                        ui.label(line);
                    }
                });

            ui.horizontal(|ui| {
                ui.label(">");
                let response = ui.text_edit_singleline(&mut self.input);
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.execute(self.input.clone());
                    self.input.clear();
                }
            });
        });
    }

    pub fn append_log(&mut self, message: &str) {
        self.output.push_back(message.to_string());
        if self.output.len() > 1000 {
            self.output.pop_front();
        }
        self.output_tx.send(message.to_string()).expect("Failed to send log message");
    }

    pub fn clear_output(&mut self) {
        self.output.clear();
    }

    pub fn execute(&self, command: String) {
        let working_dir = self.working_directory.clone();
        let output_tx = self.output_tx.clone();
        thread::spawn(move || {
            let dir = working_dir.lock().unwrap().clone();
            execute_command(&command, output_tx, dir);
        });
    }
}

fn execute_command(command: &str, output_tx: Sender<String>, working_dir: Option<PathBuf>) {
    let mut process_command = if cfg!(target_os = "windows") {
        let mut cmd = Command::new("cmd");
        cmd.args(&["/C", command]);
        cmd
    } else {
        let mut cmd = Command::new("sh");
        cmd.arg("-c").arg(command);
        cmd
    };

    if let Some(dir) = working_dir.as_ref() {
        process_command.current_dir(dir);
    }

    // Log the command being executed with the working directory
    let _ = output_tx.send(format!("Executing command: {} in directory: {:?}", command, working_dir));

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
            let output_tx_clone = output_tx.clone();
            thread::spawn(move || {
                for line in stderr_reader.lines() {
                    if let Ok(line) = line {
                        output_tx_clone.send(line).expect("Failed to send stderr line");
                    }
                }
            });
        }

        let _ = process.wait(); // Wait for the process to finish
    } else {
        let _ = output_tx.send("Failed to execute command".to_string());
    }
}
