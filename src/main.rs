#[allow(unused_imports)]
use eframe::egui;
use zzz::core::ide::IDE;

fn main() -> eframe::Result<()> {
    let options = eframe::NativeOptions::default();
    eframe::run_native(
        "ZZZ IDE",
        options,
        Box::new(|cc| Ok(Box::new(IDE::new(cc))))
    )
}
