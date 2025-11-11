pub mod api;
pub mod app;
pub mod importer;
pub mod models;

use eframe::{self, egui};

pub use app::GraphchanApp;

/// Launches the egui application with default window options.
pub fn run_frontend() -> Result<(), eframe::Error> {
    run_frontend_with_options(default_native_options())
}

/// Launches the egui app with caller-provided options.
pub fn run_frontend_with_options(options: eframe::NativeOptions) -> Result<(), eframe::Error> {
    let _ = env_logger::builder().is_test(false).try_init();
    eframe::run_native(
        "Graphchan Frontend",
        options,
        Box::new(|cc| Box::new(GraphchanApp::new(cc))),
    )
}

fn default_native_options() -> eframe::NativeOptions {
    eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    }
}
