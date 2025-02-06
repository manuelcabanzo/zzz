use std::path::PathBuf;
use eframe::egui;
use crate::{core::{git_manager::{GitCommit, GitManager}, ide::IDE}, utils::themes::{custom_theme, Theme}};

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsTab {
    Personalization,
    AI,
    Git,
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
    git_manager: Option<GitManager>,
    commits: Vec<GitCommit>,
    selected_commit: Option<String>,
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
            git_manager: None,
            commits: Vec::new(),
            selected_commit: None,
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

    pub fn update_git_manager(&mut self, project_path: Option<PathBuf>) {
        self.commits.clear();
        self.git_manager = None;
        if let Some(path) = project_path {
            println!("Updating git manager for path: {}", path.display()); // Add this
            let git_manager = GitManager::new(path.clone());

            if !git_manager.is_git_repo() {
                println!("Path is not a git repository: {}", path.display()); // Add this
                return;
            }
            match git_manager.get_commits() {
                Ok(commits) => {
                    println!("Found {} commits", commits.len()); // Add this
                    self.commits = commits;
                    self.git_manager = Some(git_manager);
                },
                Err(e) => {
                    println!("Error getting commits: {}", e); // Add this
                    self.commits.clear();
                }
            }
        }
    }

    fn show_git_settings(&mut self, ide: &mut IDE, ui: &mut egui::Ui) {
        ui.heading("Git History");
        ui.add_space(10.0);
        if let Some(git_manager) = &self.git_manager {
            if let Ok(commits) = git_manager.get_commits() {
                for commit in commits {
                    ui.horizontal(|ui| {
                        ui.label(&commit.hash);
                        if ui.button("Reset to This Commit").clicked() {
                            match git_manager.reset_to_commit(&commit.hash) {
                                Ok(()) => {
                                    // Force reload of filesystem
                                    ide.file_modal.reload_file_system();
                                    // Reload all open buffers
                                    ide.code_editor.reload_all_buffers(
                                        &ide.file_modal.file_system.as_ref().unwrap(),
                                        &mut |msg| ide.console_panel.log(msg)
                                    );
                                    ide.console_panel.log(
                                        &format!("Successfully reset to commit {}", commit.hash)
                                    );
                                },
                                Err(e) => ide.console_panel.log(&e),
                            }
                        }
                    });
                }
            }
        } else {
            ui.label("No Git repository found in the current project.");
        }
    }

    pub fn show(&mut self, ctx: &egui::Context, ide: &mut IDE) {
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
                    ui.selectable_value(&mut self.settings_tab, SettingsTab::Git, "Git");
                });
                match self.settings_tab {
                    SettingsTab::Personalization => self.show_personalization_settings(ui, ctx),
                    SettingsTab::AI => self.show_ai_settings(ui),
                    SettingsTab::Git => self.show_git_settings(ide, ui),
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