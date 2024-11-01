use eframe::egui;
use egui::{Vec2, Rect, Pos2, Color32, Stroke};
use std::sync::{Arc, Mutex};
use serde::{Serialize, Deserialize};
use std::process::Command;
use std::path::PathBuf;
use std::time::Duration;
use std::thread;
use reqwest::blocking::Client;

// Define missing enums
#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum DeviceType {
    Phone,
    Tablet,
}

#[derive(Clone, Copy, Serialize, Deserialize)]
pub enum Orientation {
    Portrait,
    Landscape,
}

// Create serialization wrapper for Color32
#[derive(Serialize, Deserialize)]
#[serde(remote = "Color32")]
struct Color32Def {
    #[serde(getter = "Color32::to_array")]
    rgba: [u8; 4],
}

impl From<Color32Def> for Color32 {
    fn from(def: Color32Def) -> Self {
        Color32::from_rgba_unmultiplied(def.rgba[0], def.rgba[1], def.rgba[2], def.rgba[3])
    }
}

// Create serialization wrapper for Pos2
#[derive(Serialize, Deserialize)]
#[serde(remote = "Pos2")]
struct Pos2Def {
    x: f32,
    y: f32,
}

impl From<Pos2Def> for Pos2 {
    fn from(def: Pos2Def) -> Self {
        Pos2::new(def.x, def.y)
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct AppPreviewState {
    #[serde(with = "Color32Def")]
    pub background_color: Color32,
    pub content: String,
    pub components: Vec<UIComponent>,
    pub layout: Layout,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct UIComponent {
    pub component_type: String,
    pub text: Option<String>,
    pub style: ComponentStyle,
    #[serde(with = "Pos2Def")]
    pub position: Pos2,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct ComponentStyle {
    #[serde(with = "Color32Def")]
    pub background_color: Color32,
    pub width: f32,
    pub height: f32,
    pub padding: f32,
    pub margin: f32,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Layout {
    pub flex_direction: FlexDirection,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum FlexDirection {
    Row,
    Column,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum JustifyContent {
    FlexStart,
    Center,
    FlexEnd,
}

#[derive(Clone, Serialize, Deserialize)]
pub enum AlignItems {
    FlexStart,
    Center,
    FlexEnd,
}

pub struct AppState {
    pub preview_state: Mutex<AppPreviewState>,
    pub metro_server: Mutex<Option<MetroServer>>,
}

#[derive(Clone, Debug)]
pub enum ServerStatus {
    Starting,
    Running,
    Failed(String),
}

// Modify your existing MetroServer struct
pub struct MetroServer {
    pub process: std::process::Child,
    pub port: u16,
    pub status: ServerStatus,  // Add this field
}

impl AppState {
    pub fn new() -> Self {
        Self {
            preview_state: Mutex::new(AppPreviewState {
                background_color: Color32::WHITE,
                content: "Loading React Native App...".to_string(),
                components: Vec::new(),
                layout: Layout {
                    flex_direction: FlexDirection::Column,
                    justify_content: JustifyContent::FlexStart,
                    align_items: AlignItems::Center,
                },
            }),
            metro_server: Mutex::new(None),
        }
    }

    pub fn start_metro_server(&self, project_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let port = 8081;
        
        // Kill any existing Metro process
        if let Some(mut server) = self.metro_server.lock().unwrap().take() {
            let _ = server.process.kill();
        }

        // Verify node_modules exists
        let node_modules_path = project_path.join("node_modules");
        #[allow(unused_variables)]
        let cli_path = node_modules_path.join(".bin").join("react-native");
        
        if !node_modules_path.exists() {
            // Run npm install if node_modules is missing
            println!("Installing dependencies...");
            let status = Command::new("npm")
                .arg("install")
                .current_dir(project_path)
                .status()?;

            if !status.success() {
                return Err("npm install failed".into());
            }
        }

        // Start Metro process using the local react-native CLI
        let process = if cfg!(windows) {
            Command::new("cmd")
                .args(["/C", "npx", "react-native", "start", "--port", &port.to_string()])
                .current_dir(project_path)
                .spawn()?
        } else {
            Command::new("npx")
                .args(["react-native", "start", "--port", &port.to_string()])
                .current_dir(project_path)
                .spawn()?
        };

        *self.metro_server.lock().unwrap() = Some(MetroServer { 
            process, 
            port,
            status: ServerStatus::Starting,
        });

        // Give Metro a moment to start
        thread::sleep(Duration::from_secs(2));
        
        Ok(())
    }

    pub fn update_from_rn_state(&self, json_state: &str) -> Result<(), serde_json::Error> {
        let new_state: AppPreviewState = serde_json::from_str(json_state)?;
        *self.preview_state.lock().unwrap() = new_state;
        Ok(())
    }
}

pub struct EmulatorPanel {
    device_type: DeviceType,
    orientation: Orientation,
    app_state: Arc<AppState>,
    scale_factor: f32,
}

impl EmulatorPanel {
    pub fn new(app_state: Arc<AppState>) -> Self {
        Self {
            device_type: DeviceType::Phone,
            orientation: Orientation::Portrait,
            app_state,
            scale_factor: 1.0,
        }
    }

    pub fn show(&mut self, ui: &mut egui::Ui) {
        ui.heading("React Native Preview");

        if let Some(server) = self.app_state.metro_server.lock().unwrap().as_ref() {
            match &server.status {
                ServerStatus::Starting => {
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label("Starting Metro server...");
                    });
                }
                ServerStatus::Running => {
                    ui.label(format!("Metro server running on port {}", server.port));
                }
                ServerStatus::Failed(error) => {
                    ui.colored_label(Color32::RED, format!("Server error: {}", error));
                }
            }
        }

        // Device controls
        ui.horizontal(|ui| {
            if ui.button("Phone").clicked() {
                self.device_type = DeviceType::Phone;
            }
            if ui.button("Tablet").clicked() {
                self.device_type = DeviceType::Tablet;
            }
            if ui.button("Rotate").clicked() {
                self.orientation = match self.orientation {
                    Orientation::Portrait => Orientation::Landscape,
                    Orientation::Landscape => Orientation::Portrait,
                };
            }
            
            ui.add(egui::Slider::new(&mut self.scale_factor, 0.5..=2.0).text("Zoom"));
        });

        let preview_state = self.app_state.preview_state.lock().unwrap();
        
        let (base_width, base_height) = match (self.device_type, self.orientation) {
            (DeviceType::Phone, Orientation::Portrait) => (360.0, 640.0),
            (DeviceType::Phone, Orientation::Landscape) => (640.0, 360.0),
            (DeviceType::Tablet, Orientation::Portrait) => (600.0, 800.0),
            (DeviceType::Tablet, Orientation::Landscape) => (800.0, 600.0),
        };

        let scaled_width = base_width * self.scale_factor;
        let scaled_height = base_height * self.scale_factor;

        let (rect, response) = ui.allocate_exact_size(
            Vec2::new(scaled_width, scaled_height),
            egui::Sense::click_and_drag(),
        );

        // Draw device frame
        ui.painter().rect_stroke(
            rect,
            10.0,
            Stroke::new(2.0, Color32::DARK_GRAY),
        );

        // Draw app background
        let content_rect = rect.shrink(4.0);
        ui.painter().rect_filled(
            content_rect,
            0.0,
            preview_state.background_color,
        );

        // Draw components
        for component in &preview_state.components {
            self.draw_component(ui, &content_rect, component);
        }

        if response.clicked() {
            if let Some(pos) = response.interact_pointer_pos() {
                let normalized_pos = self.normalize_position(pos, content_rect);
                self.handle_touch(normalized_pos);
            }
        }

        // Display touch coordinates
        if let Some(pos) = response.hover_pos() {
            let normalized_pos = self.normalize_position(pos, content_rect);
            ui.label(format!(
                "Touch position: ({:.2}, {:.2})",
                normalized_pos.x,
                normalized_pos.y
            ));
        }
    }

    fn draw_component(&self, ui: &mut egui::Ui, parent_rect: &Rect, component: &UIComponent) {
        let rect = Rect::from_min_size(
            parent_rect.min + component.position.to_vec2(),
            Vec2::new(component.style.width, component.style.height),
        );

        // Draw component background
        ui.painter().rect_filled(
            rect,
            0.0,
            component.style.background_color,
        );

        // Draw component text if any
        if let Some(text) = &component.text {
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::TextStyle::Body.resolve(ui.style()),
                Color32::BLACK,
            );
        }
    }

    fn normalize_position(&self, pos: Pos2, rect: Rect) -> Pos2 {
        Pos2::new(
            (pos.x - rect.min.x) / rect.width(),
            (pos.y - rect.min.y) / rect.height(),
        )
    }

    fn handle_touch(&self, pos: Pos2) {
        // Send touch event to React Native app
        println!("Touch event at: ({:.2}, {:.2})", pos.x, pos.y);
    }
}