mod api;
mod app;
// mod image_loader; // removed legacy loader
mod importer;
mod models;

fn main() -> Result<(), eframe::Error> {
    env_logger::init();
    let native_options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([1100.0, 720.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };
    eframe::run_native(
        "Graphchan Frontend",
        native_options,
        Box::new(|cc| Box::new(app::GraphchanApp::new(cc))),
    )
}
