use std::collections::VecDeque;
use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio, Child};
use std::thread;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicBool, Ordering};
use crossbeam_channel::{unbounded, Sender, Receiver};
#[cfg(windows)]
use std::os::windows::io::AsRawHandle;

pub struct Terminal {
    input: String,
    output: VecDeque<String>,
    output_rx: Receiver<String>,
    working_directory: Arc<Mutex<Option<PathBuf>>>,
    output_tx: Sender<String>,
    current_process: Arc<Mutex<Option<Child>>>,
    is_running: Arc<AtomicBool>,
}

impl Terminal {
    pub fn new() -> (Self, Receiver<String>) {
        let (output_tx, output_rx) = unbounded::<String>();
        let working_directory = Arc::new(Mutex::new(None));
        let current_process = Arc::new(Mutex::new(None));
        let is_running = Arc::new(AtomicBool::new(false));

        (Self {
            input: String::new(),
            output: VecDeque::new(),
            output_rx: output_rx.clone(),
            working_directory,
            output_tx,
            current_process,
            is_running,
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

    
    pub fn send_ctrl_c(&self) {
        self.stop_current_process();
    }

    fn stop_current_process(&self) {
        if let Some(child) = self.current_process.lock().unwrap().take() {
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGINT);
            }
            #[cfg(windows)]
            {
                use winapi::um::processthreadsapi::TerminateProcess;
                use winapi::um::winnt::HANDLE;
                unsafe {
                    TerminateProcess(child.as_raw_handle() as HANDLE, 1);
                }
            }
            self.is_running.store(false, Ordering::SeqCst);
            self.output_tx.send("Process stopped".to_string()).expect("Failed to send stop message");
        }
    }

    pub fn execute(&self, command: String) {
        if command.trim() == "stop" {
            self.stop_current_process();
            return;
        }

        let working_dir = self.working_directory.clone();
        let output_tx = self.output_tx.clone();
        let current_process = self.current_process.clone();
        let is_running = self.is_running.clone();
        
        thread::spawn(move || {
            is_running.store(true, Ordering::SeqCst);
            let dir = working_dir.lock().unwrap().clone();
            
            let mut process_command = if cfg!(target_os = "windows") {
                let mut cmd = Command::new("cmd");
                cmd.args(&["/C", &command]);
                cmd
            } else {
                let mut cmd = Command::new("sh");
                cmd.arg("-c").arg(&command);
                cmd
            };

            if let Some(dir) = dir.as_ref() {
                process_command.current_dir(dir);
            }

            let _ = output_tx.send(format!("Executing command: {} in directory: {:?}", command, dir));

            let process = process_command
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match process {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();

                    // Store the child process
                    *current_process.lock().unwrap() = Some(child);

                    if let Some(stdout) = stdout {
                        let stdout_reader = std::io::BufReader::new(stdout);
                        let output_tx_clone = output_tx.clone();
                        thread::spawn(move || {
                            for line in stdout_reader.lines() {
                                if let Ok(line) = line {
                                    let _ = output_tx_clone.send(line);
                                }
                            }
                        });
                    }

                    if let Some(stderr) = stderr {
                        let stderr_reader = std::io::BufReader::new(stderr);
                        let output_tx_clone = output_tx.clone();
                        thread::spawn(move || {
                            for line in stderr_reader.lines() {
                                if let Ok(line) = line {
                                    let _ = output_tx_clone.send(line);
                                }
                            }
                        });
                    }

                    // Wait for the process to finish
                    if let Some(mut child) = current_process.lock().unwrap().take() {
                        let _ = child.wait();
                    }

                    is_running.store(false, Ordering::SeqCst);
                },
                Err(e) => {
                    let _ = output_tx.send(format!("Failed to execute command: {}", e));
                    is_running.store(false, Ordering::SeqCst);
                }
            }
        });
    }
}

fn execute_command(command: &str, output_tx: Sender<String>, working_dir: Option<PathBuf>, current_process: Arc<Mutex<Option<Child>>>, is_running: Arc<AtomicBool>) {
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

    let _ = output_tx.send(format!("Executing command: {} in directory: {:?}", command, working_dir));

    let process = process_command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();

    if let Ok(child) = process {
        *current_process.lock().unwrap() = Some(child);

        if let Some(child) = &mut *current_process.lock().unwrap() {
            if let Some(stdout) = child.stdout.take() {
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

            if let Some(stderr) = child.stderr.take() {
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

            let _ = child.wait(); // Wait for the process to finish
            is_running.store(false, Ordering::SeqCst);
        }
        
        *current_process.lock().unwrap() = None;
    } else {
        let _ = output_tx.send("Failed to execute command".to_string());
        is_running.store(false, Ordering::SeqCst);
    }
}
