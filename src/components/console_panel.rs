use eframe::egui;
use crate::core::terminal::Terminal;

pub struct ConsolePanel {
    pub console_output: String,
    terminal: Terminal,
}

impl ConsolePanel {
    pub fn new() -> Self {
        Self {
            console_output: String::new(),
            terminal: Terminal::new(),
        }
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("console_panel")
            .resizable(true)
            .default_height(250.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Console");
                    if ui.button("Clear").clicked() {
                        self.console_output.clear();
                    }
                });

                egui::TopBottomPanel::bottom("terminal")
                    .resizable(true)
                    .default_height(150.0)
                    .show_inside(ui, |ui| {
                        ui.heading("Terminal");
                        self.terminal.render(ui);
                    });

                let scroll_area = egui::ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .stick_to_bottom(true);
                scroll_area.show(ui, |ui| {
                    ui.label(&self.console_output);
                });
            });
    }

    pub fn log(&mut self, message: &str) {
        self.console_output.push_str(message);
        self.console_output.push('\n');
        println!("{}", message);
    }

    pub fn update(&mut self) {
        self.terminal.update();
    }
}
