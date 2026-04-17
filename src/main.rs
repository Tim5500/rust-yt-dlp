mod app;
mod backend;
mod config;

use app::RustYtdlpApp;
use eframe::NativeOptions;

fn main() -> eframe::Result {
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1050.0, 720.0])
            .with_resizable(true),
        ..Default::default()
    };

    eframe::run_native(
        "rust-yt-dlp",
        options,
        Box::new(|cc| Ok(Box::new(RustYtdlpApp::new(cc)))),
    )
}