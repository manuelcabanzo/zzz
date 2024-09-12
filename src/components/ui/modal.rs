use eframe::egui;

pub struct Modal {
    pub show: bool,
    pub title: String,
    pub content: Box<dyn Fn(&mut egui::Ui)>,
    pub on_close: Box<dyn Fn()>,
}

impl Modal {
    pub fn new<F, C>(title: &str, content: F, on_close: C) -> Self
    where
        F: Fn(&mut egui::Ui) + 'static,
        C: Fn() + 'static,
    {
        Self {
            show: false,
            title: title.to_string(),
            content: Box::new(content),
            on_close: Box::new(on_close),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        if self.show {
            let modal = egui::Window::new(&self.title)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);

            modal.show(ctx, |ui| {
                (self.content)(ui);

                ui.with_layout(egui::Layout::bottom_up(egui::Align::Center), |ui| {
                    if ui.button("Close").clicked() {
                        self.show = false;
                        (self.on_close)();
                    }
                });
            });
        }
    }
}
