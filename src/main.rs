#[allow(unused_imports)]
use eframe::egui;
use eframe::egui::IconData;
use image::ImageReader;
use image::GenericImageView;
use zzz::core::ide::IDE;
use std::path::PathBuf;
use std::sync::Arc;
use eframe::HardwareAcceleration;

fn main() -> eframe::Result<()> {
    let icon_path = PathBuf::from("src/resources/blacksquare.png");
    let icon = if icon_path.exists() {
        let img = ImageReader::open(icon_path)
            .expect("Failed to open icon file")
            .decode()
            .expect("Failed to decode icon");
        let rgba = img.to_rgba8();
        let (width, height) = img.dimensions();
        Some(Arc::new(IconData {
            rgba: rgba.into_raw(),
            width: width as _,
            height: height as _,
        }))
    } else {
        eprintln!("Icon file not found");
        None
    };
    
    let mut viewport = egui::ViewportBuilder::default()
        .with_title("ZZZ")
        .with_app_id("Mobile Dev IDE")
        .with_inner_size([800.0, 600.0])
        .with_decorations(false)
        .with_resizable(true)
        .with_maximize_button(true);

    if let Some(icon) = icon {
        viewport = viewport.with_icon(icon);
    }
    
    let native_options = eframe::NativeOptions {
        viewport,
        vsync: true,                        // Enable VSync for smoother rendering
        multisampling: 4,                   // Use 4x multisampling for better rendering quality
        hardware_acceleration: HardwareAcceleration::Preferred, // Prefer GPU acceleration
        centered: true,
        ..Default::default()
    };
    
    eframe::run_native(
        "Mobile Dev IDE",
        native_options,
        Box::new(|cc| Ok(Box::new(IDE::new(cc))))
    )
}
