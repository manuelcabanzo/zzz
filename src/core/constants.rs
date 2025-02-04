use image::{load_from_memory, GenericImageView};
use std::sync::Arc;
use eframe::egui::IconData;

pub struct AppConstants {
    pub icon: Arc<IconData>,
    // Add other constants here
}

impl AppConstants {
    pub fn load() -> Self {
        Self {
            icon: Self::load_icon(),
            // Initialize other constants
        }
    }

    fn load_icon() -> Arc<IconData> {
        let icon_data = include_bytes!("../resources/icons/app.png");
        println!("Loaded icon size: {} bytes", icon_data.len());

        let img = load_from_memory(icon_data)
            .expect("Failed to load embedded icon");
        
        let rgba = img.to_rgba8();
        let (width, height) = img.dimensions();
    
        Arc::new(IconData {
            rgba: rgba.into_raw(),
            width,
            height,
        })
    }
}