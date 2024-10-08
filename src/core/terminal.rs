use std::collections::VecDeque;
use std::process::Stdio;
use std::path::PathBuf;
use std::sync::{Arc, Mutex, atomic::{AtomicBool, Ordering}};
use crossbeam_channel::{unbounded, Sender, Receiver};
use tokio::task;
use tokio::time::Duration;
use tokio::process::{Command as TokioCommand, Child};
use tokio::io::{BufReader, AsyncBufReadExt};
use tokio::sync::broadcast;

pub struct Terminal {
    input: String,
    output: Arc<Mutex<VecDeque<String>>>,
    output_rx: Receiver<String>,
    working_directory: Arc<Mutex<PathBuf>>,
    output_tx: Sender<String>,
    current_process: Arc<Mutex<Option<Child>>>,
    pub is_running: Arc<AtomicBool>,
    runtime: Arc<tokio::runtime::Runtime>,
    stop_tx: broadcast::Sender<()>,
}

impl Terminal {
    pub fn new(runtime: Arc<tokio::runtime::Runtime>) -> (Self, Receiver<String>) {
        let (output_tx, output_rx) = unbounded::<String>();
        let working_directory = Arc::new(Mutex::new(PathBuf::from("/")));
        let current_process = Arc::new(Mutex::new(None));
        let is_running = Arc::new(AtomicBool::new(false));
        let (stop_tx, _) = broadcast::channel(1);

        let terminal = Self {
            input: String::new(),
            output: Arc::new(Mutex::new(VecDeque::new())),
            output_rx: output_rx.clone(),
            output_tx,
            working_directory,
            current_process,
            is_running,
            runtime,
            stop_tx,
        };
        (terminal, output_rx)
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
    
    
    pub fn clear_output(&self) {
        self.output.lock().unwrap().clear();
        // Send a special message to indicate clearing the console
        self.output_tx.send("__CLEAR_CONSOLE__".to_string()).expect("Failed to send clear message");
    }
    
    pub fn execute(&self, command: String) {
        let working_dir = self.working_directory.clone();
        let output_tx = self.output_tx.clone();
        let is_running = self.is_running.clone();
        let runtime = self.runtime.clone();
        let current_process = self.current_process.clone();
        let mut stop_rx = self.stop_tx.subscribe();

        runtime.spawn(async move {
            is_running.store(true, Ordering::SeqCst);
            let dir = working_dir.lock().unwrap().clone();

            let mut process_command = if cfg!(target_os = "windows") {
                let mut cmd = TokioCommand::new("cmd");
                cmd.args(&["/C", &command])
                   .current_dir(&dir)
                   .kill_on_drop(true)
                   .stdout(Stdio::piped())
                   .stderr(Stdio::piped());
                cmd
            } else {
                let mut cmd = TokioCommand::new("sh");
                cmd.arg("-c")
                   .arg(&command)
                   .current_dir(&dir)
                   .kill_on_drop(true)
                   .stdout(Stdio::piped())
                   .stderr(Stdio::piped());
                cmd
            };

            let _ = output_tx.send(format!("Executing command: {} in directory: {:?}", command, dir));

            match process_command.spawn() {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();
                    
                    {
                        let mut current_process_lock = current_process.lock().unwrap();
                        *current_process_lock = Some(child);
                    }
                    
                    if let Some(stdout) = stdout {
                        let output_tx_clone = output_tx.clone();
                        task::spawn(async move {
                            let mut reader = BufReader::new(stdout).lines();
                            while let Some(line) = reader.next_line().await.unwrap_or(None) {
                                let _ = output_tx_clone.send(line);
                            }
                        });
                    }

                    if let Some(stderr) = stderr {
                        let output_tx_clone = output_tx.clone();
                        task::spawn(async move {
                            let mut reader = BufReader::new(stderr).lines();
                            while let Some(line) = reader.next_line().await.unwrap_or(None) {
                                let _ = output_tx_clone.send(line);
                            }
                        });
                    }

                    let status = tokio::select! {
                        status = async {
                            loop {
                                let child_option = {
                                    let mut lock = current_process.lock().unwrap();
                                    lock.as_mut().map(|child| child.try_wait())
                                };

                                if let Some(Ok(Some(status))) = child_option {
                                    return Ok(status);
                                }

                                tokio::time::sleep(Duration::from_millis(100)).await;
                            }
                        } => status,
                        _ = stop_rx.recv() => {
                            let child_option = current_process.lock().unwrap().take();
                            if let Some(mut child) = child_option {
                                let _ = child.kill().await;
                                child.wait().await
                            } else {
                                Ok(std::process::ExitStatus::default())
                            }
                        },
                        _ = tokio::time::sleep(Duration::from_secs(3600)) => {
                            Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "Process timed out after 1 hour"))
                        }
                    };

                    match status {
                        Ok(status) => {
                            let _ = output_tx.send(format!("Process exited with status: {}", status));
                        }
                        Err(e) => {
                            let _ = output_tx.send(format!("Error waiting for process: {}", e));
                        }
                    }

                    let mut current_process_lock = current_process.lock().unwrap();
                    *current_process_lock = None;
                    is_running.store(false, Ordering::SeqCst);
                },
                Err(e) => {
                    let _ = output_tx.send(format!("Failed to execute command: {}", e));
                    is_running.store(false, Ordering::SeqCst);
                }
            }
        });
    }

    pub fn stop_current_process(&self) {
        let _ = self.stop_tx.send(());
        let _ = self.output_tx.send("Stopping the current process...".to_string());
    }
}
