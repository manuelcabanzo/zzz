use eframe::egui;

pub struct ConsolePanel {
    pub console_output: String,
}

impl ConsolePanel {
    pub fn new() -> Self {
        Self {
            console_output: String::new(),
        }
    }

    pub fn show(&self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("console_panel")
            .resizable(false)
            .default_height(250.0)
            .show(ctx, |ui| {
                ui.heading("Console");
                let scroll_area = egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true);
                scroll_area.show(ui, |ui| {
                    let lines: Vec<_> = self.console_output.lines().collect();
                    let text = lines.join("\n");
                    ui.label(text);
                });
            });
    }

    pub fn log(&mut self, message: &str) {
        self.console_output.push_str(message);
        self.console_output.push('\n');
        println!("{}", message);
    }
}
