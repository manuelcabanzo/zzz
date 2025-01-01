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
use tokio::runtime::Runtime;
use std::sync::Mutex;

fn main() -> eframe::Result<()> {
    // Create a runtime to be shared across the application
    let runtime = Arc::new(Runtime::new().expect("Failed to create Tokio runtime"));
    
    // Create channels for shutdown coordination
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    
    // Wrap shutdown_rx in an Arc<Mutex> so it can be safely shared
    let shutdown_rx = Arc::new(Mutex::new(Some(shutdown_rx)));
    
    // Initialize LSP manager without blocking
    let lsp_manager = Arc::new(TokioMutex::new(Some(LspManager::new())));
    
    // Clone references for spawning LSP initialization
    let lsp_manager_clone = Arc::clone(&lsp_manager);
    
    // Spawn LSP server initialization as a separate task
    runtime.spawn(async move {
        if let Some(lsp) = lsp_manager_clone.lock().await.as_mut() {
            if let Err(e) = lsp.start_server().await {
                eprintln!("Failed to start LSP server: {}", e);
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

    let runtime_clone = runtime.clone();
    let result = eframe::run_native(
        "Mobile Dev IDE",
        native_options,
        Box::new(move |cc| {
            let mut ide = IDE::new(cc, lsp_manager.clone());
            ide.tokio_runtime = runtime_clone; // Make sure this line exists
            ide.shutdown_sender = Some(shutdown_tx);
            Ok(Box::new(ide))
        }),
    );

    // Handle shutdown in a synchronous context
    if let Ok(mut guard) = shutdown_rx.lock() {
        if let Some(rx) = guard.take() {
            // Use a new runtime for shutdown handling to avoid nested runtime issues
            let shutdown_runtime = Runtime::new().expect("Failed to create shutdown runtime");
            shutdown_runtime.block_on(async {
                let _ = rx.await;
            });
        }
    }

    // Clean up runtime in a synchronous context
    drop(runtime);

    result
}