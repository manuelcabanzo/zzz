use eframe::egui;

pub struct EmulatorPanel;

impl EmulatorPanel {
    pub fn new() -> Self {
        Self
    }

    pub fn show(&self, ctx: &egui::Context) {
        egui::SidePanel::right("emulator_panel")
            .resizable(false)
            .exact_width(500.0)
            .show(ctx, |ui| {
                ui.heading("Emulator");
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label("Emulator goes here");
                });
            });
    }
}
