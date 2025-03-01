use eframe::egui::{self, Button, RichText, Ui};
use std::process::Command;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::runtime::Runtime;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct EmulatorPanel {
    scrcpy_running: bool,
    device_connected: Arc<AtomicBool>,
    scrcpy_process: Option<std::process::Child>,
    project_path: Option<PathBuf>,
    last_build_status: Arc<Mutex<Option<String>>>,
    runtime: Arc<Runtime>,
    app_package_name: String,
    app_activity_name: String,
    is_initializing: Arc<AtomicBool>,
    is_building: Arc<AtomicBool>,
    scrcpy_path: Option<PathBuf>,
}

impl EmulatorPanel {
    pub fn new() -> Self {
        let panel = EmulatorPanel {
            scrcpy_running: false,
            device_connected: Arc::new(AtomicBool::new(false)),
            scrcpy_process: None,
            project_path: None,
            last_build_status: Arc::new(Mutex::new(None)),
            runtime: Arc::new(Runtime::new().expect("Failed to create Tokio runtime")),
            app_package_name: String::new(),
            app_activity_name: String::new(),
            is_initializing: Arc::new(AtomicBool::new(true)),
            is_building: Arc::new(AtomicBool::new(false)),
            scrcpy_path: Self::find_scrcpy_path(),
        };

        panel.initialize();
        panel
    }

    /// Attempt to locate the `scrcpy` executable.
    fn find_scrcpy_path() -> Option<PathBuf> {
        let paths = [
            PathBuf::from("src/resources/scrcpy/scrcpy.exe"), // Windows
            PathBuf::from("/usr/local/bin/scrcpy"),           // macOS/Linux
            PathBuf::from("/usr/bin/scrcpy"),                 // Linux
        ];

        paths.into_iter().find(|path| path.exists())
    }

    /// Update project path from FileModal.
    pub fn update_from_file_modal(&mut self, file_modal_project_path: Option<PathBuf>) {
        if let Some(path) = file_modal_project_path {
            if self.project_path.as_ref() != Some(&path) {
                self.set_project_path(path);
            }
        }
    }

    /// Validate and set the project path.
    pub fn set_project_path(&mut self, path: PathBuf) {
        if !self.validate_project_structure(&path) {
            return;
        }

        self.project_path = Some(path.clone());

        // Extract package and activity info from the project.
        if let Some((package_name, activity_name)) = self.extract_manifest_info() {
            self.app_package_name = package_name;
            self.app_activity_name = activity_name;
            self.update_status(Some(format!(
                "Project configured. Package: {}, Activity: {}",
                self.app_package_name, self.app_activity_name
            )));
        } else if let Some(package_name) = self.extract_package_from_gradle() {
            self.app_package_name = package_name.clone();
            self.app_activity_name = format!("{}.MainActivity", package_name);
            self.update_status(Some(
                "Package name found in gradle, but please verify the activity name is correct."
                    .to_string(),
            ));
        } else {
            self.update_status(Some(
                "Could not detect package info. Please configure manually.".to_string(),
            ));
        }
    }

    /// Validate the Android project structure.
    fn validate_project_structure(&self, path: &Path) -> bool {
        let gradle_wrapper = if cfg!(windows) {
            path.join("gradlew.bat")
        } else {
            path.join("gradlew")
        };

        let has_app_dir = path.join("app").exists();
        let has_gradle_wrapper = gradle_wrapper.exists();
        let has_gradle_dir = path.join("gradle").exists();
        let has_build_gradle = path.join("app/build.gradle.kts").exists() || path.join("app/build.gradle").exists();
        let has_settings_gradle = path.join("settings.gradle.kts").exists() || path.join("settings.gradle").exists();

        if !has_app_dir {
            self.update_status(Some("Invalid project structure: 'app' directory not found".to_string()));
            return false;
        }

        if !has_gradle_wrapper {
            self.update_status(Some("Invalid project structure: Gradle wrapper not found".to_string()));
            return false;
        }

        if !has_gradle_dir {
            self.update_status(Some("Invalid project structure: 'gradle' directory not found".to_string()));
            return false;
        }

        if !has_build_gradle {
            self.update_status(Some("Invalid project structure: build.gradle not found".to_string()));
            return false;
        }

        if !has_settings_gradle {
            self.update_status(Some("Invalid project structure: settings.gradle not found".to_string()));
            return false;
        }

        true
    }

    /// Extract package and activity info from AndroidManifest.xml.
    fn extract_manifest_info(&self) -> Option<(String, String)> {
        if let Some(path) = &self.project_path {
            let manifest_path = path.join("app/src/main/AndroidManifest.xml");

            if let Ok(content) = std::fs::read_to_string(&manifest_path) {
                let mut package_name = String::new();
                let mut activity_name = String::new();

                if let Some(pkg_start) = content.find("package=\"") {
                    if let Some(pkg_end) = content[pkg_start + 9..].find('\"') {
                        package_name = content[pkg_start + 9..pkg_start + 9 + pkg_end].to_string();
                    }
                }

                if let Some(activity_start) = content.find("android:name=\"") {
                    if let Some(activity_end) = content[activity_start + 13..].find('\"') {
                        activity_name = content[activity_start + 13..activity_start + 13 + activity_end].to_string();
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

    /// Extract package name from build.gradle.
    fn extract_package_from_gradle(&self) -> Option<String> {
        if let Some(path) = &self.project_path {
            let build_gradle_path = path.join("app/build.gradle");
            let build_gradle_kts_path = path.join("app/build.gradle.kts");

            let content = std::fs::read_to_string(&build_gradle_path)
                .or_else(|_| std::fs::read_to_string(&build_gradle_kts_path))
                .ok()?;

            for line in content.lines() {
                if line.contains("applicationId") || line.contains("namespace") {
                    return line.split("\"").nth(1).map(|s| s.to_string());
                }
            }
        }

        None
    }

    /// Initialize the panel by checking for connected devices.
    fn initialize(&self) {
        let runtime = self.runtime.clone();
        let device_connected = self.device_connected.clone();
        let is_initializing = self.is_initializing.clone();

        std::thread::spawn(move || {
            runtime.block_on(async {
                if let Ok(output) = Command::new("adb").args(["devices"]).output() {
                    let devices = String::from_utf8_lossy(&output.stdout);
                    device_connected.store(devices.lines().count() > 1, Ordering::SeqCst);
                }
                is_initializing.store(false, Ordering::SeqCst);
            });
        });
    }

    /// Check for connected devices.
    fn check_device_connection(&self) {
        let runtime = self.runtime.clone();
        let device_connected = self.device_connected.clone();

        runtime.spawn(async move {
            if let Ok(output) = Command::new("adb").args(["devices"]).output() {
                let devices = String::from_utf8_lossy(&output.stdout);
                device_connected.store(devices.lines().count() > 1, Ordering::SeqCst);
            }
        });
    }

    /// Start `scrcpy` for screen mirroring.
    fn start_scrcpy(&mut self) {
        if let Some(scrcpy_path) = &self.scrcpy_path {
            match Command::new(scrcpy_path).arg("--tcpip").spawn() {
                Ok(child) => {
                    self.scrcpy_process = Some(child);
                    self.scrcpy_running = true;
                    self.update_status(Some("Screen mirroring started".to_string()));
                }
                Err(e) => {
                    self.update_status(Some(format!("Failed to start scrcpy: {}", e)));
                }
            }
        } else {
            self.update_status(Some("scrcpy executable not found".to_string()));
        }
    }

    /// Stop `scrcpy`.
    fn stop_scrcpy(&mut self) {
        if let Some(mut process) = self.scrcpy_process.take() {
            let _ = process.kill();
            self.scrcpy_running = false;
            self.update_status(Some("Screen mirroring stopped".to_string()));
        }
    }

    /// Run the app with screen mirroring.
    fn run_app_with_mirror(&mut self) {
        self.check_device_connection();

        if !self.device_connected.load(Ordering::SeqCst) {
            self.update_status(Some("No device connected".to_string()));
            return;
        }

        if self.project_path.is_none() {
            self.update_status(Some("Please select an Android project directory first".to_string()));
            return;
        }

        if !self.scrcpy_running {
            self.start_scrcpy();
            if !self.scrcpy_running {
                self.update_status(Some("Failed to start screen mirroring. Continuing with app deployment.".to_string()));
            }
        }

        let runtime_handle = self.runtime.handle().clone();
        let project_path = self.project_path.clone();
        let package_name = self.app_package_name.clone();
        let activity_name = self.app_activity_name.clone();
        let build_status = Arc::clone(&self.last_build_status);
        let is_building = Arc::clone(&self.is_building);

        is_building.store(true, Ordering::SeqCst);

        runtime_handle.spawn(async move {
            let mut status = build_status.lock().unwrap();

            *status = Some("Building Android app...".to_string());
            match Self::build_app(&project_path) {
                Ok(_) => {
                    *status = Some("Build successful, installing app...".to_string());
                    match Self::install_app(&project_path) {
                        Ok(_) => {
                            *status = Some("Installation successful, launching app...".to_string());
                            match Self::launch_app(&package_name, &activity_name) {
                                Ok(msg) => *status = Some(msg),
                                Err(e) => *status = Some(format!("Launch failed: {}", e)),
                            }
                        }
                        Err(e) => *status = Some(format!("Installation failed: {}", e)),
                    }
                }
                Err(e) => *status = Some(format!("Build failed: {}", e)),
            }

            is_building.store(false, Ordering::SeqCst);
        });

        self.update_status(Some("Starting app deployment process...".to_string()));
    }

    /// Build the app using Gradle.
    fn build_app(project_path: &Option<PathBuf>) -> Result<String, String> {
        let path = project_path.as_ref().ok_or("No project path set")?;
        
        let gradle_wrapper = if cfg!(windows) {
            path.join("gradlew.bat")
        } else {
            path.join("gradlew")
        };

        println!("Building app at path: {}", path.display());
        println!("Using gradle wrapper: {}", gradle_wrapper.display());

        let build_result = Command::new(&gradle_wrapper)
            .arg("assembleDebug")
            .current_dir(path)
            .output()
            .map_err(|e| format!("Failed to execute gradle command: {}", e))?;

        let stdout = String::from_utf8_lossy(&build_result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&build_result.stderr).to_string();

        println!("Build stdout: {}", stdout);
        if !stderr.is_empty() {
            println!("Build stderr: {}", stderr);
        }

        if !build_result.status.success() {
            return Err(format!("Build failed:\nStdout: {}\nStderr: {}", stdout, stderr));
        }

        Ok("Build successful".to_string())
    }

    /// Install the app on the connected device.
    fn install_app(project_path: &Option<PathBuf>) -> Result<String, String> {
        let path = project_path.as_ref().ok_or("No project path set")?;
        let apk_path = path.join("app/build/outputs/apk/debug/app-debug.apk");

        println!("Installing APK from: {}", apk_path.display());

        if !apk_path.exists() {
            return Err(format!("APK not found at {:?}. Make sure the build was successful.", apk_path));
        }

        println!("Running adb install command...");
        let install_result = Command::new("adb")
            .args(["install", "-r", apk_path.to_str().unwrap()])
            .output()
            .map_err(|e| format!("Installation failed: {}", e))?;

        let stdout = String::from_utf8_lossy(&install_result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&install_result.stderr).to_string();

        println!("Install stdout: {}", stdout);
        if !stderr.is_empty() {
            println!("Install stderr: {}", stderr);
        }

        if !install_result.status.success() {
            return Err(format!("Installation failed:\nStdout: {}\nStderr: {}", stdout, stderr));
        }

        Ok("Installation successful".to_string())
    }

    /// Launch the app on the connected device.
    fn launch_app(package_name: &str, activity_name: &str) -> Result<String, String> {
        let launch_result = Command::new("adb")
            .args(["shell", "am", "start", "-n", &format!("{}/{}", package_name, activity_name)])
            .output()
            .map_err(|e| format!("App launch failed: {}", e))?;

        let stdout = String::from_utf8_lossy(&launch_result.stdout).to_string();
        let stderr = String::from_utf8_lossy(&launch_result.stderr).to_string();

        if !launch_result.status.success() {
            return Err(format!("Launch failed:\nStdout: {}\nStderr: {}", stdout, stderr));
        }

        Ok("App launched successfully".to_string())
    }

    /// Update the build status.
    fn update_status(&self, status: Option<String>) {
        let mut last_status = self.last_build_status.lock().unwrap();
        *last_status = status;
    }

    /// Render the UI.
    pub fn show(&mut self, ui: &mut Ui) {
        ui.heading("App Runner");

        if self.is_initializing.load(Ordering::SeqCst) {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label("Initializing emulator panel...");
            });
            return;
        }

        // Project path status
        if self.project_path.is_none() {
            ui.label(RichText::new("No Android project selected").color(egui::Color32::YELLOW));
        } else {
            ui.label(RichText::new(format!("Project: {:?}", self.project_path.as_ref().unwrap())).color(egui::Color32::GREEN));
            ui.label(format!("Package: {}", self.app_package_name));
            ui.label(format!("Activity: {}", self.app_activity_name));
        }

        // Device connection status
        ui.horizontal(|ui| {
            let is_connected = self.device_connected.load(Ordering::SeqCst);

            let status_text = if is_connected {
                RichText::new("Device Connected").color(egui::Color32::GREEN)
            } else {
                RichText::new("No Device Connected").color(egui::Color32::RED)
            };
            ui.label(status_text);

            if ui.button("Refresh").clicked() {
                self.check_device_connection();
            }
        });

        // Run controls
        if !self.scrcpy_running {
            let is_connected = self.device_connected.load(Ordering::SeqCst);
            if ui.add_enabled(
                is_connected && self.project_path.is_some(),
                Button::new("▶ Run App")
            ).clicked() {
                self.run_app_with_mirror();
            }
        } else {
            if ui.button("⏹ Stop").clicked() {
                self.stop_scrcpy();
            }
        }

        // Build status
        if let Some(status) = &*self.last_build_status.lock().unwrap() {
            ui.label(RichText::new(status).color(
                if status.contains("successful") || status.contains("started") {
                    egui::Color32::GREEN
                } else {
                    egui::Color32::RED
                }
            ));
        }

        // App configuration
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