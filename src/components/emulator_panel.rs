use eframe::egui::{self, Button, RichText, Ui};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;

pub struct EmulatorPanel {
    scrcpy_running: bool,
    device_connected: bool,
    scrcpy_process: Option<std::process::Child>,
    project_path: Option<PathBuf>,
    last_build_status: Option<String>,
    runtime: Runtime,
    app_package_name: String,
    app_activity_name: String,
}

impl EmulatorPanel {
    pub fn new() -> Self {
        EmulatorPanel {
            scrcpy_running: false,
            device_connected: false,
            scrcpy_process: None,
            project_path: None,
            last_build_status: None,
            runtime: Runtime::new().expect("Failed to create Tokio runtime"),
            app_package_name: String::new(),
            app_activity_name: String::new(),
        }
    }

    // Update project path from FileModal
    pub fn update_from_file_modal(&mut self, file_modal_project_path: Option<PathBuf>) {
        if let Some(path) = file_modal_project_path {
            if self.project_path.as_ref() != Some(&path) {
                self.set_project_path(path);
            }
        }
    }

    pub fn set_project_path(&mut self, path: PathBuf) {
        // More flexible validation for Android Studio project structure
        let gradle_wrapper = if cfg!(windows) {
            path.join("gradlew.bat")
        } else {
            path.join("gradlew")
        };
    
        // Check for essential Android project files and directories
        let has_app_dir = path.join("app").exists();
        let has_gradle_wrapper = gradle_wrapper.exists();
        let has_gradle_dir = path.join("gradle").exists();
        let has_build_gradle = path.join("app/build.gradle.kts").exists() || path.join("app/build.gradle").exists();
        let has_settings_gradle = path.join("settings.gradle.kts").exists() || path.join("settings.gradle").exists();
    
        // More detailed validation with specific feedback
        if !has_app_dir {
            self.last_build_status = Some("Invalid project structure: 'app' directory not found".to_string());
            return;
        }
    
        if !has_gradle_wrapper {
            self.last_build_status = Some("Invalid project structure: Gradle wrapper (gradlew/gradlew.bat) not found".to_string());
            return;
        }
    
        if !has_gradle_dir {
            self.last_build_status = Some("Invalid project structure: 'gradle' directory not found".to_string());
            return;
        }
    
        if !has_build_gradle {
            self.last_build_status = Some("Invalid project structure: build.gradle or build.gradle.kts not found in app directory".to_string());
            return;
        }
    
        if !has_settings_gradle {
            self.last_build_status = Some("Invalid project structure: settings.gradle or settings.gradle.kts not found".to_string());
            return;
        }
    
        self.project_path = Some(path.clone());
        // Try to get info from manifest first
        if let Some((package_name, activity_name)) = self.extract_manifest_info() {
            self.app_package_name = package_name;
            self.app_activity_name = activity_name;
            self.last_build_status = Some(format!(
                "Project configured. Package: {}, Activity: {}", 
                self.app_package_name, 
                self.app_activity_name
            ));
        } else {
            // Fallback to gradle file
            if let Some(package_name) = self.extract_package_from_gradle() {
                self.app_package_name = package_name.clone();
                // For activity, we'll assume .MainActivity but warn the user
                self.app_activity_name = format!("{}.MainActivity", package_name);
                self.last_build_status = Some(
                    "Package name found in gradle, but please verify the activity name is correct."
                    .to_string()
                );
            } else {
                self.last_build_status = Some(
                    "Could not detect package info. Please configure manually."
                    .to_string()
                );
            }
        }
    }

    pub fn configure_app(&mut self, package_name: String, activity_name: String) {
        self.app_package_name = package_name;
        self.app_activity_name = activity_name;
    }

    fn extract_package_from_gradle(&self) -> Option<String> {
        if let Some(path) = &self.project_path {
            let build_gradle_path = path.join("app/build.gradle");
            let build_gradle_kts_path = path.join("app/build.gradle.kts");
            
            // Try reading from build.gradle or build.gradle.kts
            let content = std::fs::read_to_string(&build_gradle_path)
                .or_else(|_| std::fs::read_to_string(&build_gradle_kts_path))
                .ok()?;

            // Look for applicationId or namespace in build.gradle
            for line in content.lines() {
                if line.contains("applicationId") || line.contains("namespace") {
                    return line.split("\"")
                        .nth(1)
                        .map(|s| s.to_string());
                }
            }
        }
        None
    }

    fn extract_manifest_info(&self) -> Option<(String, String)> {
        if let Some(path) = &self.project_path {
            let manifest_path = path.join("app/src/main/AndroidManifest.xml");
            
            if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                // Very basic XML parsing - in production you'd want to use a proper XML parser
                let mut package_name = String::new();
                let mut activity_name = String::new();

                // Try to find package name
                if let Some(pkg_start) = content.find("package=\"") {
                    if let Some(pkg_end) = content[pkg_start + 9..].find('\"') {
                        package_name = content[pkg_start + 9..pkg_start + 9 + pkg_end].to_string();
                    }
                }

                // Try to find main activity
                if let Some(activity_start) = content.find("android:name=\"") {
                    if let Some(activity_end) = content[activity_start + 13..].find('\"') {
                        activity_name = content[activity_start + 13..activity_start + 13 + activity_end].to_string();
                        // If activity starts with a dot, it's relative to package
                        if activity_name.starts_with('.') {
                            activity_name = format!("{}{}", package_name, activity_name);
                        }
                    }
                }

                if !package_name.is_empty() && !activity_name.is_empty() {
                    return Some((package_name, activity_name));
                }
            }
        }
        None
    }

    fn check_device_connection(&mut self) {
        let output = match Command::new("adb")
            .args(["devices"])
            .output() {
                Ok(output) => output,
                Err(e) => {
                    self.device_connected = false;
                    self.last_build_status = Some(format!("ADB error: {}", e));
                    return;
                }
            };

        let devices = String::from_utf8_lossy(&output.stdout);
        self.device_connected = devices.lines().count() > 1;
    }

    fn build_app(project_path: &Option<PathBuf>) -> Result<String, String> {
        let path = project_path.as_ref().ok_or("No project path set")?;
        
        if !path.exists() {
            return Err("Project path does not exist".to_string());
        }

        let gradle_wrapper = if cfg!(windows) {
            path.join("gradlew.bat")
        } else {
            path.join("gradlew")
        };

        println!("Building project at {:?}", path);
        println!("Using gradle wrapper at {:?}", gradle_wrapper);

        let build_result = Command::new(&gradle_wrapper)
            .arg("assembleDebug")
            .current_dir(path)
            .output()
            .map_err(|e| format!("Failed to execute gradle command: {}", e))?;

        let stdout = String::from_utf8_lossy(&build_result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&build_result.stderr).to_string();

        println!("Build stdout: {}", stdout);
        println!("Build stderr: {}", stderr);

        if !build_result.status.success() {
            return Err(format!("Build failed:\nStdout: {}\nStderr: {}", stdout, stderr));
        }

        Ok("Build successful".to_string())
    }
    
    fn install_app(project_path: &Option<PathBuf>) -> Result<String, String> {
        let path = project_path.as_ref().ok_or("No project path set")?;
        let apk_path = path.join("app/build/outputs/apk/debug/app-debug.apk");
    
        println!("Looking for APK at {:?}", apk_path);
        if !apk_path.exists() {
            return Err(format!("APK not found at {:?}. Make sure the build was successful.", apk_path));
        }
    
        println!("Installing APK...");
        let install_result = Command::new("adb")
            .args(["install", "-r", apk_path.to_str().unwrap()])
            .output()
            .map_err(|e| format!("Installation failed: {}", e))?;
    
        let stdout = String::from_utf8_lossy(&install_result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&install_result.stderr).to_string();

        println!("Install stdout: {}", stdout);
        println!("Install stderr: {}", stderr);

        if !install_result.status.success() {
            return Err(format!("Installation failed:\nStdout: {}\nStderr: {}", stdout, stderr));
        }
    
        Ok("Installation successful".to_string())
    }
    
    fn launch_app(package_name: &str, activity_name: &str) -> Result<String, String> {
        println!("Launching app: {}/{}", package_name, activity_name);
        let launch_result = Command::new("adb")
            .args([
                "shell", 
                "am", 
                "start", 
                "-n", 
                &format!("{}/{}", package_name, activity_name)
            ])
            .output()
            .map_err(|e| format!("App launch failed: {}", e))?;
    
        let stdout = String::from_utf8_lossy(&launch_result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&launch_result.stderr).to_string();

        println!("Launch stdout: {}", stdout);
        println!("Launch stderr: {}", stderr);

        if !launch_result.status.success() {
            return Err(format!("Launch failed:\nStdout: {}\nStderr: {}", stdout, stderr));
        }
    
        Ok("App launched successfully".to_string())
    }

    fn start_scrcpy(&mut self) {
        let scrcpy_path = Path::new("src/resources/scrcpy/scrcpy.exe");
        
        if !scrcpy_path.exists() {
            self.last_build_status = Some("scrcpy executable not found".to_string());
            return;
        }

        match Command::new(scrcpy_path)
            .arg("--tcpip")
            .spawn() {
                Ok(child) => {
                    self.scrcpy_process = Some(child);
                    self.scrcpy_running = true;
                    self.last_build_status = Some("Screen mirroring started".to_string());
                }
                Err(e) => {
                    self.last_build_status = Some(format!("Failed to start scrcpy: {}", e));
                }
            }
    }

    fn stop_scrcpy(&mut self) {
        if let Some(mut process) = self.scrcpy_process.take() {
            let _ = process.kill();
            self.scrcpy_running = false;
            self.last_build_status = Some("Screen mirroring stopped".to_string());
        }
    }

    fn run_app_with_mirror(&mut self) {
        self.check_device_connection();
        
        if !self.device_connected {
            self.last_build_status = Some("No device connected".to_string());
            return;
        }

        if self.project_path.is_none() {
            self.last_build_status = Some("Please select an Android project directory first".to_string());
            return;
        }

        // Create thread-safe status for async operations
        let runtime_handle = self.runtime.handle().clone();
        let project_path = self.project_path.clone();
        let package_name = self.app_package_name.clone();
        let activity_name = self.app_activity_name.clone();
        let build_status = Arc::new(Mutex::new(String::new()));
        let status_clone = build_status.clone();

        // Run build and deployment in async task
        runtime_handle.spawn(async move {
            let mut status = build_status.lock().unwrap();
            
            // Build app
            *status = "Building app...".to_string();
            match Self::build_app(&project_path) {
                Ok(_) => {
                    // Install app
                    *status = "Installing app...".to_string();
                    match Self::install_app(&project_path) {
                        Ok(_) => {
                            // Launch app
                            *status = "Launching app...".to_string();
                            match Self::launch_app(&package_name, &activity_name) {
                                Ok(msg) => *status = msg,
                                Err(e) => *status = format!("Launch failed: {}", e),
                            }
                        },
                        Err(e) => *status = format!("Installation failed: {}", e),
                    }
                },
                Err(e) => *status = format!("Build failed: {}", e),
            }
        });

        // Update build status after async operation
        if let Ok(status) = status_clone.lock() {
            self.last_build_status = Some(status.clone());
        }

        // Start screen mirroring only after successful deployment
        if self.last_build_status.as_ref().map_or(false, |s| s.contains("successful")) {
            self.start_scrcpy();
        }
    }

    pub fn show(&mut self, ui: &mut Ui) {
        ui.heading("App Runner");
        
        ui.add_space(8.0);
        
        // Project path status
        if self.project_path.is_none() {
            ui.label(RichText::new("No Android project selected").color(egui::Color32::YELLOW));
        } else {
            ui.label(RichText::new(format!("Project: {:?}", self.project_path.as_ref().unwrap())).color(egui::Color32::GREEN));
            
            // Show package name and activity
            ui.label(format!("Package: {}", self.app_package_name));
            ui.label(format!("Activity: {}", self.app_activity_name));
        }
        
        // Device connection status
        ui.horizontal(|ui| {
            self.check_device_connection();

            let status_text = if self.device_connected {
                RichText::new("Device Connected").color(egui::Color32::GREEN)
            } else {
                RichText::new("No Device Connected").color(egui::Color32::RED)
            };
            ui.label(status_text);
            
            if ui.button("Refresh").clicked() {
                self.check_device_connection();
            }
        });

        ui.add_space(8.0);

        // Run controls
        if !self.scrcpy_running {
            if ui.add_enabled(
                self.device_connected && self.project_path.is_some(),
                Button::new("▶ Run App")
            ).clicked() {
                self.run_app_with_mirror();
            }
        } else {
            if ui.button("⏹ Stop").clicked() {
                self.stop_scrcpy();
            }
        }

        ui.add_space(16.0);

        // Build status with more details
        if let Some(status) = &self.last_build_status {
            ui.label(RichText::new(status).color(
                if status.contains("successful") || status.contains("started") {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                }
            ));
        }

        // Add configuration fields for package and activity names
        ui.add_space(16.0);
        ui.group(|ui| {
            ui.label("App Configuration");
            let mut package_name = self.app_package_name.clone();
            if ui.text_edit_singleline(&mut package_name).changed() {
                self.app_package_name = package_name;
            }
            
            let mut activity_name = self.app_activity_name.clone();
            if ui.text_edit_singleline(&mut activity_name).changed() {
                self.app_activity_name = activity_name;
            }
        });
    }
}

impl Drop for EmulatorPanel {
    fn drop(&mut self) {
        self.stop_scrcpy();
    }
}