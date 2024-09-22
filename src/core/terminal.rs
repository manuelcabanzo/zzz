use std::collections::VecDeque;
use std::io::BufRead;
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
    output: Arc<Mutex<VecDeque<String>>>,
    output_rx: Receiver<String>,
    working_directory: Arc<Mutex<PathBuf>>,
    output_tx: Sender<String>,
    current_process: Arc<Mutex<Option<Child>>>,
    is_running: Arc<AtomicBool>,
}

impl Terminal {
    pub fn new() -> (Self, Receiver<String>) {
        let (output_tx, output_rx) = unbounded::<String>();
        let working_directory = Arc::new(Mutex::new(PathBuf::from("/")));
        let current_process = Arc::new(Mutex::new(None));
        let is_running = Arc::new(AtomicBool::new(false));

        (Self {
            input: String::new(),
            output: Arc::new(Mutex::new(VecDeque::new())),
            output_rx: output_rx.clone(),
            working_directory,
            output_tx,
            current_process,
            is_running,
        }, output_rx)
    }
    
    pub fn set_working_directory(&mut self, path: PathBuf) {
        let mut working_dir = self.working_directory.lock().unwrap();
        *working_dir = path.clone();
        // Log the new working directory
        self.output_tx.send(format!("Working directory set to: {:?}", path)).expect("Failed to send log message");
    }

    pub fn get_working_directory(&self) -> PathBuf {
        self.working_directory.lock().unwrap().clone()
    }

    pub fn update(&mut self) {
        while let Ok(output) = self.output_rx.try_recv() {
            let mut output_lock = self.output.lock().unwrap();
            output_lock.push_back(output);
            if output_lock.len() > 1000 {
                output_lock.pop_front();
            }
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            egui::ScrollArea::vertical()
                .max_height(280.0)
                .show(ui, |ui| {
                    let output_lock = self.output.lock().unwrap();
                    for line in output_lock.iter() {
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
        let mut output_lock = self.output.lock().unwrap();
        output_lock.push_back(message.to_string());
        if output_lock.len() > 1000 {
            output_lock.pop_front();
        }
        self.output_tx.send(message.to_string()).expect("Failed to send log message");
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
    
    pub fn clear_output(&self) {
        self.output.lock().unwrap().clear();
        // Send a special message to indicate clearing the console
        self.output_tx.send("__CLEAR_CONSOLE__".to_string()).expect("Failed to send clear message");
    }
    
    pub fn execute(&self, command: String) {
        if command.trim() == "stop" {
            self.stop_current_process();
            return;
        }

        if command.trim() == "clear" {
            self.clear_output();
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

            if let Some(dir_str) = dir.to_str() {
                process_command.current_dir(dir_str);
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

