use eframe::egui;
use eframe::egui::IconData;
use eframe::HardwareAcceleration;
use image::{GenericImageView, ImageReader};
use std::path::PathBuf;
use std::sync::Arc;
use zzz::core::ide::IDE;

fn main() -> eframe::Result<()> {
    let icon = load_icon("src/resources/blacksquare.png");

    let viewport = egui::ViewportBuilder::default()
        .with_title("Mobile Dev IDE")
        .with_app_id("Mobile Dev IDE")
        .with_inner_size([800.0, 600.0])
        .with_decorations(false)
        .with_resizable(true)
        .with_icon(icon.unwrap_or_default());

    let native_options = eframe::NativeOptions {
        viewport,
        vsync: true,
        multisampling: 4,
        hardware_acceleration: HardwareAcceleration::Preferred,
        centered: true,
        ..Default::default()
    };

    eframe::run_native(
        "Mobile Dev IDE",
        native_options,
        Box::new(|cc| {
            let ide = IDE::new(cc);
            Ok(Box::new(ide))
        }),
    )
}

fn load_icon(path: impl Into<PathBuf>) -> Option<Arc<IconData>> {
    let path = path.into();
    let img = match ImageReader::open(&path) {
        Ok(reader) => match reader.decode() {
            Ok(img) => img,
            Err(e) => {
                eprintln!("Failed to decode icon at '{}': {e}", path.display());
                return None;
            }
        },
        Err(e) => {
            eprintln!("Failed to open icon file at '{}': {e}", path.display());
            return None;
        }
    };

    let rgba = img.to_rgba8();
    let (width, height) = img.dimensions();

    Some(Arc::new(IconData {
        rgba: rgba.into_raw(),
        width,
        height,
    }))
}