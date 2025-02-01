use std::path::PathBuf;
use std::process::{Command, Child, Stdio};
use std::io::{BufRead, BufReader, Write};
use std::sync::{Arc, Mutex};
use crossbeam_channel::{unbounded, Sender, Receiver};
use eframe::egui::{self, Color32, text::LayoutJob};
use regex::Regex;
use std::sync::atomic::{AtomicBool, Ordering};
use syntect::easy::HighlightLines;
use syntect::highlighting::{ThemeSet, Style as SyntectStyle};
use syntect::parsing::{SyntaxSet, SyntaxReference};
use lazy_static::lazy_static;

lazy_static! {
    static ref URL_REGEX: Regex = Regex::new(r"https?://\S+").unwrap();
}

#[derive(Clone)]
struct TerminalLine {
    text: String,
    style: LineStyle,
}

#[derive(Clone)]
enum LineStyle {
    Default,
    Command,
    Success,
    Error,
    Warning,
    Link(String),
    Highlight(Vec<(SyntectStyle, String)>),
}

pub struct Terminal {
    pub current_directory: Arc<Mutex<PathBuf>>,
    input: String,
    output: Arc<Mutex<Vec<TerminalLine>>>,
    command_history: Arc<Mutex<Vec<String>>>,
    history_index: Option<usize>,
    child_process: Option<Child>,
    stdin_tx: Option<Sender<String>>,
    stdout_rx: Option<Receiver<String>>,
    running: Arc<AtomicBool>,
    auto_complete_suggestions: Arc<Mutex<Vec<String>>>,
    
    // Syntax highlighting resources
    syntax_set: SyntaxSet,
    theme_set: ThemeSet,
}

impl Terminal {
    pub fn new(initial_path: PathBuf) -> Self {
        let (stdin_tx, stdin_rx) = unbounded();
        let (stdout_tx, stdout_rx) = unbounded();
        let running = Arc::new(AtomicBool::new(true));
        
        let mut terminal = Self {
            current_directory: Arc::new(Mutex::new(initial_path.clone())),
            input: String::new(),
            output: Arc::new(Mutex::new(Vec::new())),
            command_history: Arc::new(Mutex::new(Vec::new())),
            history_index: None,
            child_process: None,
            stdin_tx: Some(stdin_tx),
            stdout_rx: Some(stdout_rx),
            running: Arc::clone(&running),
            auto_complete_suggestions: Arc::new(Mutex::new(Vec::new())),
            syntax_set: SyntaxSet::load_defaults_newlines(),
            theme_set: ThemeSet::load_defaults(),
        };

        terminal.spawn_shell();
        terminal.start_io_threads(stdin_rx, stdout_tx);
        terminal
    }

    fn spawn_shell(&mut self) {
        let mut cmd = if cfg!(target_os = "windows") {
            Command::new("cmd")
        } else {
            Command::new("/bin/bash")
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
        let stderr = child.stderr.take().expect("Failed to open stderr");

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

        // Stdout and Stderr thread
        std::thread::spawn(move || {
            let stdout_reader = BufReader::new(stdout);
            let stderr_reader = BufReader::new(stderr);

            let combined_reader = stdout_reader.lines()
                .chain(stderr_reader.lines())
                .filter_map(Result::ok);

            for line in combined_reader {
                if !running_stdout.load(Ordering::SeqCst) {
                    break;
                }
                if stdout_tx.send(line).is_err() {
                    break;
                }
            }
        });
    }

    pub fn handle_keyboard_shortcuts(&mut self, ctx: &egui::Context) {
        ctx.input(|i| {
            if i.key_pressed(egui::Key::C) && i.modifiers.ctrl {
                self.send_interrupt();
            }
        });
    }

    pub fn send_interrupt(&mut self) {
        if let Some(child) = &mut self.child_process {
            if let Some(mut stdin) = child.stdin.take() {
                let _ = stdin.write_all(&[3]); // ASCII code for Ctrl+C
                let _ = stdin.flush();
            }

            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                
                let pid = child.id() as i32;
                let _ = kill(Pid::from_raw(pid), Some(Signal::SIGINT));
            }

            #[cfg(windows)]
            {
                use winapi::um::wincon::GenerateConsoleCtrlEvent;
                let _ = unsafe { GenerateConsoleCtrlEvent(0, child.id()) };
            }
        }

        self.output.lock().unwrap().push(TerminalLine {
            text: "Interrupt signal sent".to_string(),
            style: LineStyle::Warning
        });
    }

    fn execute_command(&mut self) {
        if self.input.is_empty() { return; }
    
        let input = self.input.clone();
        let current_dir = self.current_directory.lock().unwrap().clone();
    
        self.command_history.lock().unwrap().push(input.clone());
        self.history_index = None;
    
        let parts: Vec<&str> = input.split_whitespace().collect();
    
        match parts[0] {
            "cd" => {
                if parts.len() > 1 {
                    let new_path = if parts[1].starts_with('/') || parts[1].contains(':') {
                        PathBuf::from(parts[1])
                    } else {
                        current_dir.join(parts[1])
                    };
    
                    match new_path.canonicalize() {
                        Ok(canonical_path) => {
                            *self.current_directory.lock().unwrap() = canonical_path.clone();
                            
                            self.output.lock().unwrap().push(TerminalLine {
                                text: format!("Changed directory to: {}", canonical_path.display()),
                                style: LineStyle::Success
                            });
    
                            if let Some(tx) = &self.stdin_tx {
                                let _ = tx.send(format!("cd \"{}\"", canonical_path.display()));
                            }
                        }
                        Err(e) => {
                            self.output.lock().unwrap().push(TerminalLine {
                                text: format!("Error changing directory: {}", e),
                                style: LineStyle::Error
                            });
                        }
                    }
                } else {
                    self.output.lock().unwrap().push(TerminalLine {
                        text: "Usage: cd <directory>".to_string(),
                        style: LineStyle::Warning
                    });
                }
            }
            "clear" => {
                self.clear();
            }
            "exit" => {
                self.exit();
            }
            _ => {
                self.output.lock().unwrap().push(TerminalLine {
                    text: format!("$ {}", input),
                    style: LineStyle::Command
                });
    
                if let Some(tx) = &self.stdin_tx {
                    if tx.send(input.clone()).is_err() {
                        self.output.lock().unwrap().push(TerminalLine {
                            text: "Failed to send command to shell".to_string(),
                            style: LineStyle::Error
                        });
                    }
                }
            }
        }
    
        self.input.clear();
        self.auto_complete_suggestions.lock().unwrap().clear();
    }

    fn detect_links_and_highlight(&self, line: &str) -> LineStyle {
        if let Some(mat) = URL_REGEX.find(line) {
            return LineStyle::Link(mat.as_str().to_string());
        }

        let syntax = self.guess_syntax(line);
        if let Some(syntax) = syntax {
            let theme = &self.theme_set.themes["Solarized (dark)"];
            let mut highlighter = HighlightLines::new(syntax, theme);
            
            if let Ok(highlighted_lines) = highlighter.highlight_line(line, &self.syntax_set) {
                return LineStyle::Highlight(
                    highlighted_lines
                        .iter()
                        .map(|(style, text)| (style.clone(), text.to_string()))
                        .collect()
                );
            }
        }

        LineStyle::Default
    }

    fn guess_syntax(&self, line: &str) -> Option<&SyntaxReference> {
        if line.contains(".rs") {
            self.syntax_set.find_syntax_by_extension("rs")
        } else if line.contains(".py") {
            self.syntax_set.find_syntax_by_extension("py")
        } else {
            None
        }
    }

    fn clear(&mut self) {
        self.output.lock().unwrap().clear();
        self.output.lock().unwrap().push(TerminalLine {
            text: "Terminal cleared".to_string(),
            style: LineStyle::Success
        });
    }
    
    pub fn exit(&mut self) {
        self.output.lock().unwrap().push(TerminalLine {
            text: "Attempting to terminate shell process...".to_string(),
            style: LineStyle::Warning
        });
    
        if let Some(mut child) = self.child_process.take() {
            self.running.store(false, Ordering::SeqCst);
    
            let termination_result = match std::env::consts::OS {
                #[cfg(unix)]
                _ => {
                    use nix::sys::signal::{kill, Signal};
                    use nix::unistd::Pid;
                    
                    let pid = child.id() as i32;
                    let kill_result = kill(Pid::from_raw(pid), Some(Signal::SIGKILL));
                    
                    match kill_result {
                        Ok(_) => "Process terminated successfully".to_string(),
                        Err(e) => format!("Error terminating process: {}", e)
                    }
                },
                #[cfg(windows)]
                _ => {
                    use std::process::Command;
                    
                    let kill_result = Command::new("taskkill")
                        .args(&["/F", "/T", "/PID", &child.id().to_string()])
                        .output();
                    
                    match kill_result {
                        Ok(output) if output.status.success() => 
                            "Process terminated successfully".to_string(),
                        Ok(output) => 
                            format!("Termination failed: {}", String::from_utf8_lossy(&output.stderr)),
                        Err(e) => 
                            format!("Error terminating process: {}", e)
                    }
                }
            };
    
            self.output.lock().unwrap().push(TerminalLine {
                text: termination_result,
                style: LineStyle::Warning
            });
    
            match child.try_wait() {
                Ok(Some(status)) => {
                    self.output.lock().unwrap().push(TerminalLine {
                        text: format!("Process exited with status: {}", status),
                        style: LineStyle::Warning
                    });
                },
                Ok(None) => {
                    self.output.lock().unwrap().push(TerminalLine {
                        text: "Process is still running".to_string(),
                        style: LineStyle::Warning
                    });
                },
                Err(e) => {
                    self.output.lock().unwrap().push(TerminalLine {
                        text: format!("Error waiting for process: {}", e),
                        style: LineStyle::Error
                    });
                }
            }
        } else {
            self.output.lock().unwrap().push(TerminalLine {
                text: "No active child process to terminate".to_string(),
                style: LineStyle::Warning
            });
        }
    }

    fn parse_and_style_output(&mut self, line: String) -> TerminalLine {
        let style = match true {
            _ if line.contains("ERROR:") => LineStyle::Error,
            _ if line.contains("warning") => LineStyle::Warning,
            _ if line.starts_with("$ ") => {
                let stripped_line = line.trim_start_matches("$ ").to_string();
                self.output.lock().unwrap().push(TerminalLine {
                    text: stripped_line.clone(),
                    style: LineStyle::Command
                });
                LineStyle::Default
            },
            _ => self.detect_links_and_highlight(&line)
        };
    
        TerminalLine { text: line, style }
    }

    pub fn update(&mut self) {
        let mut new_lines = Vec::new();
        
        if let Some(rx) = self.stdout_rx.clone() {
            while let Ok(line) = rx.try_recv() {
                let styled_line = self.parse_and_style_output(line);
                new_lines.push(styled_line);
            }
        }
        
        self.output.lock().unwrap().extend(new_lines);
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
                    for line in self.output.lock().unwrap().iter() {
                        self.render_terminal_line(line, ui);
                    }
                });

            ui.horizontal(|ui| {
                let response = ui.text_edit_singleline(&mut self.input);
                
                ui.input(|i| {
                    if i.key_pressed(egui::Key::ArrowUp) {
                        self.navigate_history(true);
                    }
                    if i.key_pressed(egui::Key::ArrowDown) {
                        self.navigate_history(false);
                    }
                });

                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    self.execute_command();
                }

                if response.changed() {
                    self.auto_complete();
                }
            });

            if !self.auto_complete_suggestions.lock().unwrap().is_empty() {
                egui::ComboBox::from_label("Suggestions")
                    .show_ui(ui, |ui| {
                        for suggestion in self.auto_complete_suggestions.lock().unwrap().iter() {
                            if ui.button(suggestion).clicked() {
                                self.input = suggestion.clone();
                            }
                        }
                    });
            }

            ui.horizontal(|ui| {
                if ui.button("Clear").clicked() {
                    self.clear();
                }
                if ui.button("Exit").clicked() {
                    self.exit();
                }
                if ui.button("Restart Shell").clicked() {
                    self.restart_shell();
                }
            });
        });
    }

    fn render_terminal_line(&self, line: &TerminalLine, ui: &mut egui::Ui) {
        match &line.style {
            LineStyle::Command => {
                ui.colored_label(Color32::LIGHT_GREEN, &line.text);
            }
            LineStyle::Error => {
                ui.colored_label(Color32::RED, &line.text);
            }
            LineStyle::Warning => {
                ui.colored_label(Color32::YELLOW, &line.text);
            }
            LineStyle::Link(url) => {
                let _ = ui.hyperlink_to(url, url);
            }
            LineStyle::Highlight(styles) => {
                let mut layout_job = LayoutJob::default();
                for (style, text) in styles {
                    let color = Color32::from_rgb(
                        style.foreground.r, 
                        style.foreground.g, 
                        style.foreground.b
                    );
                    layout_job.append(
                        &text, 
                        0.0, 
                        egui::TextFormat::simple(
                            egui::FontId::proportional(12.0), 
                            color
                        )
                    );
                }
                ui.label(layout_job);
            }
            LineStyle::Default => {
                ui.label(&line.text);
            }
            LineStyle::Success => {
                ui.colored_label(Color32::GREEN, &line.text);
            }
        }
    }

    fn navigate_history(&mut self, previous: bool) {
        let command_history = self.command_history.lock().unwrap();
        if command_history.is_empty() {
            return;
        }

        match self.history_index {
            None => {
                self.history_index = Some(if previous {
                    command_history.len() - 1
                } else {
                    0
                });
            }
            Some(index) => {
                if previous && index > 0 {
                    self.history_index = Some(index - 1);
                } else if !previous && index < command_history.len() - 1 {
                    self.history_index = Some(index + 1);
                }
            }
        }

        if let Some(index) = self.history_index {
            self.input = command_history[index].clone();
        }
    }

    fn auto_complete(&mut self) {
        let current_input = self.input.clone();
        let suggestions = vec![
            "cd".to_string(),
            "ls".to_string(),
            "pwd".to_string(),
            "git".to_string(),
            "clear".to_string(),
            "exit".to_string(),
        ].into_iter()
         .filter(|cmd| cmd.starts_with(&current_input))
         .collect::<Vec<_>>();

        *self.auto_complete_suggestions.lock().unwrap() = suggestions;
    }

    pub fn add_output(&mut self, message: String) {
        self.output.lock().unwrap().push(TerminalLine {
            text: message,
            style: LineStyle::Default
        });
    }

    fn restart_shell(&mut self) {
        self.exit();
    
        self.running = Arc::new(AtomicBool::new(true));
    
        let current_project_path = self.current_directory.lock().unwrap().clone();
    
        self.current_directory = Arc::new(Mutex::new(current_project_path.clone()));
    
        let (stdin_tx, stdin_rx) = unbounded();
        let (stdout_tx, stdout_rx) = unbounded();
    
        let mut cmd = if cfg!(target_os = "windows") {
            Command::new("cmd")
        } else {
            Command::new("/bin/bash")
        };
    
        cmd.current_dir(current_project_path.clone())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
    
        let child_process = cmd.spawn().expect("Failed to spawn shell");
        
        self.child_process = Some(child_process);
        self.stdin_tx = Some(stdin_tx);
        self.stdout_rx = Some(stdout_rx);
        self.start_io_threads(stdin_rx, stdout_tx);
    
        self.output.lock().unwrap().push(TerminalLine {
            text: format!("Shell restarted successfully in: {}", current_project_path.display()),
            style: LineStyle::Success
        });
    }
}