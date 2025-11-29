use eframe::egui;
use crate::app::{GraphchanApp, messages::AppMessage};

pub fn render_import_page(app: &mut GraphchanApp, ui: &mut egui::Ui) {
    ui.heading("Import Thread");
    ui.add_space(10.0);

    ui.columns(2, |columns| {
        // Column 1: 4chan
        columns[0].vertical(|ui| {
            ui.heading("4chan");
            ui.label("Enter a 4chan thread URL to import it into Graphchan.");
            ui.add_space(5.0);
            
            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.text_edit_singleline(&mut app.importer.url);
            });

            if let Some(error) = &app.importer.error {
                ui.colored_label(egui::Color32::RED, error);
            }

            if ui.add_enabled(!app.importer.importing, egui::Button::new("Import 4chan Thread")).clicked() {
                app.importer.importing = true;
                app.importer.error = None;
                crate::app::tasks::import_fourchan(
                    app.api.clone(),
                    app.tx.clone(),
                    app.importer.url.clone(),
                );
            }
        });

        // Column 2: Reddit
        columns[1].vertical(|ui| {
            ui.heading("Reddit");
            ui.label("Enter a Reddit thread URL to import it.");
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("URL:");
                ui.text_edit_singleline(&mut app.reddit_importer.url);
            });

            if let Some(error) = &app.reddit_importer.error {
                ui.colored_label(egui::Color32::RED, error);
            }

            if ui.add_enabled(!app.reddit_importer.importing, egui::Button::new("Import Reddit Thread")).clicked() {
                app.reddit_importer.importing = true;
                app.reddit_importer.error = None;
                crate::app::tasks::import_reddit(
                    app.api.clone(),
                    app.tx.clone(),
                    app.reddit_importer.url.clone(),
                );
            }
        });
    });
}
