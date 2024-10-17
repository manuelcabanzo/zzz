use std::process::{Command, Child, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use crossbeam_channel::{unbounded, Sender, Receiver};
use eframe::egui;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct Terminal {
    pub current_directory: Arc<Mutex<PathBuf>>,
    input: String,
    pub output: Vec<String>,
    child_process: Option<Child>,
    stdin_tx: Option<Sender<String>>,
    stdout_rx: Option<Receiver<String>>,
    running: Arc<AtomicBool>,
}

impl Terminal {
    pub fn new(initial_path: PathBuf) -> Self {
        let (stdin_tx, stdin_rx) = unbounded();
        let (stdout_tx, stdout_rx) = unbounded();
        let running = Arc::new(AtomicBool::new(true));
        
        let mut terminal = Self {
            current_directory: Arc::new(Mutex::new(initial_path)),
            input: String::new(),
            output: Vec::new(),
            child_process: None,
            stdin_tx: Some(stdin_tx),
            stdout_rx: Some(stdout_rx),
            running: Arc::clone(&running),
        };

        terminal.spawn_shell();
        terminal.start_io_threads(stdin_rx, stdout_tx);

        terminal
    }

    fn spawn_shell(&mut self) {
        let mut cmd = if cfg!(target_os = "windows") {
            Command::new("cmd")
        } else {
            Command::new("sh")
        };

        cmd.current_dir(self.current_directory.lock().unwrap().clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        self.child_process = Some(cmd.spawn().expect("Failed to spawn shell"));
    }

    fn start_io_threads(&mut self, stdin_rx: Receiver<String>, stdout_tx: Sender<String>) {
        let child = self.child_process.as_mut().expect("Child process not initialized");
        let running_stdin = Arc::clone(&self.running);
        let running_stdout = Arc::clone(&self.running);

        let mut stdin = child.stdin.take().expect("Failed to open stdin");
        let stdout = child.stdout.take().expect("Failed to open stdout");

        // Stdin thread
        std::thread::spawn(move || {
            for input in stdin_rx {
                if !running_stdin.load(Ordering::SeqCst) {
                    break;
                }
                if writeln!(stdin, "{}", input).is_err() {
                    break;
                }
            }
        });

        // Stdout thread
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                if !running_stdout.load(Ordering::SeqCst) {
                    break;
                }
                if let Ok(line) = line {
                    if stdout_tx.send(line).is_err() {
                        break;
                    }
                }
            }
        });
    }

    pub fn update(&mut self) {
        if let Some(rx) = &self.stdout_rx {
            while let Ok(line) = rx.try_recv() {
                self.output.push(line);
            }
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.vertical(|ui| {
            ui.heading("Terminal");

            let current_dir = self.current_directory.lock().unwrap().clone();
            ui.label(format!("Current Directory: {}", current_dir.display()));

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

            let response = ui.text_edit_singleline(&mut self.input);
            if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                self.execute_command();
            }

            ui.horizontal(|ui| {
                if ui.button("Clear").clicked() {
                    self.clear();
                }
                if ui.button("Exit").clicked() {
                    self.exit();
                }
                if ui.button("Ctrl+C").clicked() {
                    self.ctrl_c();
                }
            });
        });
    }

    fn execute_command(&mut self) {
        if let Some(tx) = &self.stdin_tx {
            tx.send(self.input.clone()).expect("Failed to send input");
        }
        self.input.clear();
    }

    fn clear(&mut self) {
        self.output.clear();
    }

    pub fn exit(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(mut child) = self.child_process.take() {
            let _ = child.kill();
        }
        self.output.push("Terminal session ended.".to_string());
    }

    pub fn ctrl_c(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(child) = self.child_process.take() {
            // Implement platform-specific interrupt here
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                let _ = kill(Pid::from_raw(child.id() as i32), Signal::SIGINT);
            }
            #[cfg(windows)]
            {
                use std::os::windows::io::AsRawHandle;
                use winapi::um::processthreadsapi::TerminateProcess;
                use winapi::um::winnt::HANDLE;
                
                unsafe {
                    TerminateProcess(child.as_raw_handle() as HANDLE, 1);
                }
            }
        }
        self.spawn_shell();
        self.running.store(true, Ordering::SeqCst);
    }
}
