use eframe::egui;
use crate::utils::themes::{Theme, custom_theme};

#[derive(Debug, Clone, PartialEq)]
pub enum SettingsTab {
    Personalization,
    General,
    Editor,
}

pub struct SettingsModal {
    pub show_settings_menu: bool,
    pub settings_tab: SettingsTab,
    pub current_theme: Theme,
}

impl SettingsModal {
    pub fn new() -> Self {
        Self {
            show_settings_menu: false,
            settings_tab: SettingsTab::Personalization,
            current_theme: Theme::default(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.show_settings_menu {
            let modal = egui::Window::new("Settings")
                .collapsible(false)
                .resizable(false)
                .fixed_size([1000.0, 600.0])
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);

            modal.show(ctx, |ui| {
                ui.heading("Settings");
                ui.add_space(10.0);

                egui::TopBottomPanel::top("settings_tabs").show_inside(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::Personalization, "Personalization");
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::General, "General");
                        ui.selectable_value(&mut self.settings_tab, SettingsTab::Editor, "Editor");
                    });
                });

                egui::CentralPanel::default().show_inside(ui, |ui| {
                    match self.settings_tab {
                        SettingsTab::Personalization => self.show_personalization_settings(ui),
                        SettingsTab::General => self.show_general_settings(ui),
                        SettingsTab::Editor => self.show_editor_settings(ui),
                    }
                });

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    ui.add_space(10.0);
                    ui.label("Press Ctrl+M to close this menu");
                });
            });
        }
    }

    fn show_personalization_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Personalization");
        ui.add_space(10.0);

        if ui.button("Cream Theme").clicked() {
            self.current_theme = Theme::cream();
            self.apply_theme(ui.ctx());
        }
        if ui.button("Black Theme").clicked() {
            self.current_theme = Theme::black();
            self.apply_theme(ui.ctx());
        }
        if ui.button("Purple Theme").clicked() {
            self.current_theme = Theme::purple();
            self.apply_theme(ui.ctx());
        }
    }

    fn show_general_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("General Settings");
        ui.add_space(10.0);
        ui.label("General settings will be added here in the future.");
    }

    fn show_editor_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("Editor Settings");
        ui.add_space(10.0);
        ui.label("Editor settings will be added here in the future.");
    }

    pub fn apply_theme(&self, ctx: &egui::Context) {
        let visuals = custom_theme(ctx, &self.current_theme);
        ctx.set_visuals(visuals);
    }
}
