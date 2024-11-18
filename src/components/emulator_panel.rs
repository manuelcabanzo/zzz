use eframe::egui::{self, Button};
use std::process::Command;
use std::path::Path;

pub struct EmulatorPanel {
    scrcpy_running: bool,
}

impl EmulatorPanel {
    pub fn new() -> Self {
        EmulatorPanel {
            scrcpy_running: false,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("Emulator Panel");

        // Button to start scrcpy
        if !self.scrcpy_running {
            if ui.add(Button::new("Start Emulator")).clicked() {
                self.run_scrcpy();
            }
        } else {
            ui.label("Emulator is running...");
        }
    }

    fn run_scrcpy(&mut self) {
        // Path to scrcpy executable relative to the project
        let scrcpy_path = Path::new("src/resources/scrcpy/scrcpy.exe");

        // Check if scrcpy exists in the path
        if !scrcpy_path.exists() {
            eprintln!("scrcpy executable not found in the expected path: {}", scrcpy_path.display());
            return;
        }

        // Run scrcpy using a command
        let status = Command::new(scrcpy_path)
            .arg("--tcpip")
            .spawn()
            .map_err(|e| {
                eprintln!("Failed to start scrcpy: {}", e);
                e
            })
            .and_then(|mut child| {
                // Ensure we correctly wait for the process to exit
                child.wait().map(|status| {
                    if status.success() {
                        self.scrcpy_running = true;
                    } else {
                        eprintln!("scrcpy process exited with status: {:?}", status);
                    }
                })
            });

        // If an error occurred during process launch or wait, handle it
        if let Err(err) = status {
            // Use Debug formatting to print the error
            eprintln!("Failed to launch scrcpy: {:?}", err);
        }
    }
    
    
}
