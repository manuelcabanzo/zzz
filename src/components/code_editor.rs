use eframe::egui;

pub struct CodeEditor {
    pub code: String,
    pub current_file: Option<String>,
}

impl CodeEditor {
    pub fn new() -> Self {
        Self {
            code: String::new(),
            current_file: None,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui, available_height: f32) {
        ui.heading("Code Editor");
        if let Some(file) = &self.current_file {
            ui.label(format!("Editing: {}", file));
        }
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                let text_edit = egui::TextEdit::multiline(&mut self.code)
                    .desired_width(f32::INFINITY);
                let frame = egui::Frame::none().inner_margin(4.0);
                frame.show(ui, |ui| {
                    ui.add_sized([ui.available_width(), available_height], text_edit);
                });
            });
    }
}
