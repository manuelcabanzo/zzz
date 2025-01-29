use eframe::egui;
use crate::utils::themes::{Theme, custom_theme};

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsTab {
    Personalization,
    // General,
    // Editor,
    AI, // Add new tab for AI settings
}

pub struct SettingsModal {
    pub show: bool,
    settings_tab: SettingsTab,
    pub current_theme: Theme,
    api_key: String, // Add field for API key
    api_key_changed: bool, // Track if API key has changed
    ai_model: String, // Add field for AI model
    ai_model_changed: bool, // Track if AI model has changed
}

impl SettingsModal {
    pub fn new() -> Self {
        Self {
            show: false,
            settings_tab: SettingsTab::Personalization,
            current_theme: Theme::default(),
            api_key: String::new(),
            api_key_changed: false,
            ai_model: "Qwen/Qwen2.5-Coder-32B-Instruct".to_string(),
            ai_model_changed: false,
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
                ui.heading("Settings");
                ui.add_space(10.0);

                egui::TopBottomPanel::top("settings_tabs").show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::Personalization, "Personalization");
                        // ui.selectable_value(&mut self.settings_tab, SettingsTab::General, "General");
                        // ui.selectable_value(&mut self.settings_tab, SettingsTab::Editor, "Editor");
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::AI, "AI Assistant"); // Add new tab
                    });
                });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    match self.settings_tab {
                        SettingsTab::Personalization => self.show_personalization_settings(ui, ctx),
                        // SettingsTab::General => self.show_general_settings(ui),
                        // SettingsTab::Editor => self.show_editor_settings(ui),
                        SettingsTab::AI => self.show_ai_settings(ui), // Add new tab handler
                    }
                });    
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
            if ui.text_edit_singleline(&mut self.ai_model).changed() {
                self.ai_model_changed = true;
            }
        });

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

    // fn show_general_settings(&mut self, ui: &mut egui::Ui) {
    //     ui.heading("General Settings");
    //     ui.add_space(10.0);
    //     ui.label("General settings will be added here in the future.");
    // }

    // fn show_editor_settings(&mut self, ui: &mut egui::Ui) {
    //     ui.heading("Editor Settings");
    //     ui.add_space(10.0);
    //     ui.label("Editor settings will be added here in the future.");
    // }

    pub fn apply_theme(&self, ctx: &egui::Context) {
        let visuals = custom_theme(ctx, &self.current_theme);
        ctx.set_visuals(visuals);
    }
}
