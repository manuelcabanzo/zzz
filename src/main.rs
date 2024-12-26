use eframe::egui;
use eframe::egui::IconData;
use image::ImageReader;
use image::GenericImageView;
use zzz::core::ide::IDE;
use std::path::PathBuf;
use std::sync::Arc;
use eframe::HardwareAcceleration;
use zzz::core::lsp::LspManager;
use tokio::sync::Mutex as TokioMutex;

#[tokio::main]
async fn main() -> eframe::Result<()> {
    // Initialize LSP manager in a thread-safe way
    let lsp_manager = Arc::new(TokioMutex::new(Some(LspManager::new())));
    
    // Start LSP server in the background
    let lsp_clone = lsp_manager.clone();
    tokio::spawn(async move {
        if let Some(manager) = lsp_clone.lock().await.as_mut() {
            if let Err(e) = manager.start_server().await {
                eprintln!("Failed to start LSP server: {}", e);
                // Continue execution even if LSP fails - the IDE should still work
            }
        }
    });

    // Load application icon
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
    
    // Configure viewport
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
    
    // Configure native options
    let native_options = eframe::NativeOptions {
        viewport,
        vsync: true,
        multisampling: 4,
        hardware_acceleration: HardwareAcceleration::Preferred,
        centered: true,
        ..Default::default()
    };
    
    // Run the application
    eframe::run_native(
        "Mobile Dev IDE",
        native_options,
        Box::new(move |cc| {
            // Pass LSP manager to IDE
            Ok(Box::new(IDE::new(cc, lsp_manager.clone())))
        })
    )
}