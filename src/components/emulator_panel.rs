use eframe::egui;
use egui::{Vec2, Rect, Pos2, Color32, Stroke};
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy)]
pub enum DeviceType {
    Phone,
    Tablet,
}

#[derive(Clone, Copy)]
pub enum Orientation {
    Portrait,
    Landscape,
}

pub struct EmulatorPanel {
    device_type: DeviceType,
    orientation: Orientation,
    pub app_state: Arc<AppState>,
}

pub struct AppState {
    pub background_color: Mutex<Color32>,
    pub content: Mutex<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            background_color: Mutex::new(Color32::WHITE),
            content: Mutex::new("Hello, Mobile World!".to_string()),
        }
    }
}

impl EmulatorPanel {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self {
            device_type: DeviceType::Phone,
            orientation: Orientation::Portrait,
            app_state,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("Emulator Preview");

        // Device type selection
        ui.horizontal(|ui| {
            if ui.button("Phone").clicked() {
                self.device_type = DeviceType::Phone;
            }
            if ui.button("Tablet").clicked() {
                self.device_type = DeviceType::Tablet;
            }
        });

        // Orientation selection
        if ui.button("Rotate Device").clicked() {
            self.orientation = match self.orientation {
                Orientation::Portrait => Orientation::Landscape,
                Orientation::Landscape => Orientation::Portrait,
            };
        }

        // Emulator display
        let (width, height) = match (self.device_type, self.orientation) {
            (DeviceType::Phone, Orientation::Portrait) => (200.0, 400.0),
            (DeviceType::Phone, Orientation::Landscape) => (400.0, 200.0),
            (DeviceType::Tablet, Orientation::Portrait) => (300.0, 400.0),
            (DeviceType::Tablet, Orientation::Landscape) => (400.0, 300.0),
        };

        let (rect, response) = ui.allocate_exact_size(Vec2::new(width, height), egui::Sense::click_and_drag());

        // Draw device frame
        ui.painter().rect_stroke(rect, 10.0, Stroke::new(2.0, Color32::DARK_GRAY));

        let background_color = *self.app_state.background_color.lock().unwrap();
        let content_rect = rect.shrink(4.0);  // Define content_rect here
        ui.painter().rect_filled(content_rect, 0.0, background_color);

        // Display app content
        let content = self.app_state.content.lock().unwrap();
        ui.painter().text(
            content_rect.min,
            egui::Align2::LEFT_TOP,
            &*content,
            egui::TextStyle::Body.resolve(ui.style()),
            Color32::BLACK,
        );

        // Handle touch/click events
        if response.clicked() {
            if let Some(click_pos) = response.interact_pointer_pos() {
                let normalized_pos = self.normalize_position(click_pos, rect);
                self.handle_touch(normalized_pos);
            }
        }

        // Display touch coordinates for developer reference
        if let Some(pos) = response.hover_pos() {
            let normalized_pos = self.normalize_position(pos, rect);
            ui.label(format!("Touch position: ({:.2}, {:.2})", normalized_pos.x, normalized_pos.y));
        }
    }

    fn normalize_position(&self, pos: Pos2, rect: Rect) -> Pos2 {
        Pos2::new(
            (pos.x - rect.min.x) / rect.width(),
            (pos.y - rect.min.y) / rect.height(),
        )
    }

    fn handle_touch(&self, pos: Pos2) {
        // This is where you would handle touch events in your app
        // For this example, we'll just print the touch position
        println!("Touch at position: ({:.2}, {:.2})", pos.x, pos.y);
    }
}