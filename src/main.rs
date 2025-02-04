use eframe::egui;
use eframe::HardwareAcceleration;
use zzz::core::constants::AppConstants;


fn main() -> eframe::Result<()> {
    // Load application constants and embedded resources
    let constants = AppConstants::load();

    // Configure viewport with embedded icon
    let viewport = egui::ViewportBuilder::default()
        .with_title("Mobile Dev IDE")
        .with_app_id("Mobile Dev IDE")
        .with_inner_size([800.0, 600.0])
        .with_decorations(false)
        .with_resizable(true)
        .with_icon(constants.icon)
        .with_drag_and_drop(true);

    // Set up native options
    let native_options = eframe::NativeOptions {
        viewport,
        vsync: true,
        multisampling: 4,
        hardware_acceleration: HardwareAcceleration::Preferred,
        centered: true,
        ..Default::default()
    };

    // Start the application
    eframe::run_native(
        "Mobile Dev IDE",
        native_options,
        Box::new(|cc| {
            // Initialize the IDE component
            let ide = zzz::core::ide::IDE::new(cc);
            Ok(Box::new(ide))
        }),
    )
}