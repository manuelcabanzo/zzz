use eframe::egui;
use crate::utils::themes::{custom_theme, Theme};
use crate::core::app_creation::AppCreation;
use crate::plugin_manager::PluginManager;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsTab {
    Personalization,
    AI,
    AppCreation, // Add new tab for app creation
    Extensions, // Add new tab for extensions
}

#[derive(Clone)]
pub struct SettingsModal {
    pub show: bool,
    settings_tab: SettingsTab,
    pub current_theme: Theme,
    api_key: String, // Add field for API key
    api_key_changed: bool, // Track if API key has changed
    ai_model: String, // Add field for AI model
    ai_model_changed: bool, // Track if AI model has changed
    app_name: String, // Add field for app name
    app_path: String, // Add field for app path
    api_level: String, // Add field for API level
    logs: Arc<Mutex<Vec<String>>>, // Add field for logs
    progress: Arc<Mutex<f32>>, // Add field for progress
    plugin_manager: Arc<Mutex<PluginManager>>, // Add field for plugin manager
}

impl SettingsModal {
    pub fn new(plugin_manager: Arc<Mutex<PluginManager>>) -> Self {
        Self {
            show: false,
            settings_tab: SettingsTab::Personalization,
            current_theme: Theme::default(),
            api_key: String::new(),
            api_key_changed: false,
            ai_model: "Qwen/Qwen2.5-Coder-32B-Instruct".to_string(),
            ai_model_changed: false,
            app_name: String::new(),
            app_path: String::new(),
            api_level: "30".to_string(), // Default API level
            logs: Arc::new(Mutex::new(Vec::new())), // Initialize logs
            progress: Arc::new(Mutex::new(0.0)), // Initialize progress
            plugin_manager,
        }
    }

    // Add getter for API key
    pub fn get_api_key(&self) -> String {
        self.api_key.clone()
    }

    // Add setter for API key
    pub fn set_api_key(&mut self, key: String) {
        self.api_key = key;
    }

    // Add method to check and reset the changed flag
    pub fn take_api_key_changed(&mut self) -> bool {
        let changed = self.api_key_changed;
        self.api_key_changed = false;
        changed
    }

    // Add getter for AI model
    pub fn get_ai_model(&self) -> String {
        self.ai_model.clone()
    }

    // Add setter for AI model
    pub fn set_ai_model(&mut self, model: String) {
        self.ai_model = model;
    }

    // Add method to check and reset the changed flag
    pub fn take_ai_model_changed(&mut self) -> bool {
        let changed = self.ai_model_changed;
        self.ai_model_changed = false;
        changed
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if !self.show {
            return;
        }
        let modal_size = egui::vec2(500.0, 500.0);
        egui::Window::new("Settings")
            .fixed_size(modal_size)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ctx, |ui| {
                ui.set_min_size(modal_size);
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.settings_tab,
                        SettingsTab::Personalization,
                        "Personalization"
                    );
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::AI, "AI Assistant");
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::AppCreation, "App Creation"); // Add new tab
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Extensions, "Extensions"); // Add new tab
                });
                match self.settings_tab {
                    SettingsTab::Personalization => self.show_personalization_settings(ui, ctx),
                    SettingsTab::AI => self.show_ai_settings(ui),
                    SettingsTab::AppCreation => self.show_app_creation_settings(ui), // Show app creation settings
                    SettingsTab::Extensions => self.show_extension_settings(ui), // Show extension settings
                }
            });
    }

    fn show_ai_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("AI Assistant Settings");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("Together AI API Key:");
            if ui.text_edit_singleline(&mut self.api_key).changed() {
                self.api_key_changed = true;
            }
        });
        ui.add_space(10.0);
        ui.horizontal(|ui| {
            ui.label("AI Model:");
            let models = vec![
                "deepseek-ai/DeepSeek-R1",
                "meta-llama/Meta-Llama-3.1-8B-Instruct-Turbo-128K",
                "Qwen/Qwen2-VL-72B-Instruct",
                "Other",
            ];
            let mut selected_model = self.ai_model.clone();
            egui::ComboBox::from_label("Select AI Model")
                .selected_text(selected_model.clone())
                .show_ui(ui, |ui| {
                    for model in &models {
                        ui.selectable_value(&mut selected_model, model.to_string(), model.to_string());
                    }
                });
            if selected_model != self.ai_model {
                if selected_model == "Other" {
                    self.ai_model = String::new();
                } else {
                    self.ai_model = selected_model;
                }
                self.ai_model_changed = true;
            }
        });
        if self.ai_model.is_empty() {
            ui.horizontal(|ui| {
                ui.label("Custom Model:");
                if ui.text_edit_singleline(&mut self.ai_model).changed() {
                    self.ai_model_changed = true;
                }
            });
        }
        ui.add_space(5.0);
        ui.label("Your API key and model are stored locally and used only for AI assistant functionality.");
    }

    fn show_personalization_settings(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        ui.heading("Personalization");
        ui.add_space(10.0);
        if ui.button("Cream Theme").clicked() {
            self.current_theme = Theme::cream();
            self.apply_theme(ctx);
        }
        if ui.button("Black Theme").clicked() {
            self.current_theme = Theme::black();
            self.apply_theme(ctx);
        }
        if ui.button("Purple Theme").clicked() {
            self.current_theme = Theme::purple();
            self.apply_theme(ctx);
        }
    }

    fn show_app_creation_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("App Creation");
        ui.add_space(10.0);

        ui.horizontal(|ui| {
            ui.label("App Name:");
            ui.text_edit_singleline(&mut self.app_name);
        });

        ui.horizontal(|ui| {
            ui.label("App Path:");
            ui.text_edit_singleline(&mut self.app_path);
        });

        ui.horizontal(|ui| {
            ui.label("API Level:");
            ui.text_edit_singleline(&mut self.api_level);
        });

        if ui.button("Create App").clicked() {
            let logs_callback = {
                let logs = self.logs.clone();
                Rc::new(move |log: String| {
                    let mut logs = logs.lock().unwrap();
                    logs.push(log);
                }) as Rc<dyn Fn(String)>
            };

            let progress_callback = {
                let progress = self.progress.clone();
                Rc::new(move |p: f32| {
                    let mut progress = progress.lock().unwrap();
                    *progress = p;
                }) as Rc<dyn Fn(f32)>
            };

            let app_creation = AppCreation::new(self.app_name.clone(), self.app_path.clone(), self.api_level.clone(), logs_callback, progress_callback);
            if let Err(e) = app_creation.create_app() {
                ui.label(format!("Failed to create app: {}", e));
            }
        }

        // Display progress bar
        let progress = self.progress.lock().unwrap();
        ui.add(egui::ProgressBar::new(*progress).show_percentage());

        // Display logs
        let logs = self.logs.lock().unwrap();
        for log in logs.iter() {
            ui.label(log);
        }
    }

    fn show_extension_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Extensions");
        ui.add_space(10.0);

        if ui.button("Load Extension").clicked() {
            // Open file picker dialog to select the plugin file
            if let Some(plugin_path) = self.file_modal.pick_file() {
                println!("Loading plugin from path: {:?}", plugin_path.display());
                let mut plugin_manager = self.plugin_manager.lock().unwrap();
                plugin_manager.install_plugin(&plugin_path);
            }
        }

        ui.add_space(10.0);
        ui.label("Loaded Extensions:");
        let plugin_manager = self.plugin_manager.lock().unwrap();
        for plugin in plugin_manager.list_plugins() {
            ui.label(plugin);
        }
    }

    pub fn apply_theme(&self, ctx: &egui::Context) {
        let visuals = custom_theme(ctx, &self.current_theme);
        ctx.set_visuals(visuals);
    }

    // Add a method to validate the API key and AI model
    pub fn validate_settings(&self) -> bool {
        // Example validation: API key should not be empty
        !self.api_key.is_empty() && !self.ai_model.is_empty()
    }
}