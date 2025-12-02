use eframe::egui;
use crate::app::GraphchanApp;
use crate::app::state::ViewState;

impl GraphchanApp {
    pub(crate) fn render_settings(&mut self, ui: &mut egui::Ui) {
        ui.heading("⚙ Settings");
        ui.add_space(20.0);

        egui::ScrollArea::vertical().show(ui, |ui| {
            // API Configuration Section
            ui.group(|ui| {
                ui.heading("API Configuration");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.label("Backend URL:");
                    ui.text_edit_singleline(&mut self.base_url_input);
                });

                ui.add_space(5.0);

                ui.horizontal(|ui| {
                    if ui.button("Apply Changes").clicked() {
                        match self.api.set_base_url(self.base_url_input.clone()) {
                            Ok(()) => {
                                self.info_banner = Some("API URL updated successfully".into());
                                self.spawn_load_threads();
                                self.view = ViewState::Catalog;
                            }
                            Err(err) => {
                                self.info_banner = Some(format!("Failed to update URL: {}", err));
                            }
                        }
                    }

                    if ui.button("Reset to Default").clicked() {
                        self.base_url_input = "http://127.0.0.1:8080".to_string();
                    }
                });

                ui.add_space(5.0);
                ui.label(egui::RichText::new("⚠ Changing the backend URL will reconnect to a different instance").small().color(egui::Color32::GRAY));
            });

            ui.add_space(20.0);

            // Keyboard Shortcuts Section
            ui.group(|ui| {
                ui.heading("Keyboard Shortcuts");
                ui.add_space(10.0);

                ui.label(egui::RichText::new("Navigation").strong());
                ui.add_space(5.0);

                egui::Grid::new("shortcuts_grid")
                    .num_columns(2)
                    .spacing([40.0, 4.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new("Tab").monospace().strong());
                        ui.label("Next post chronologically");
                        ui.end_row();

                        ui.label(egui::RichText::new("Shift+Tab").monospace().strong());
                        ui.label("Previous post chronologically");
                        ui.end_row();

                        ui.label(egui::RichText::new("Space").monospace().strong());
                        ui.label("Select post at cursor");
                        ui.end_row();

                        ui.label(egui::RichText::new("u, p").monospace().strong());
                        ui.label("Move cursor through parent posts (blue)");
                        ui.end_row();

                        ui.label(egui::RichText::new("i").monospace().strong());
                        ui.label("View parent at cursor");
                        ui.end_row();

                        ui.label(egui::RichText::new("j, l, ;").monospace().strong());
                        ui.label("Move cursor through reply posts (red)");
                        ui.end_row();

                        ui.label(egui::RichText::new("k").monospace().strong());
                        ui.label("View reply at cursor");
                        ui.end_row();
                    });
            });

            ui.add_space(20.0);

            // About Section
            ui.group(|ui| {
                ui.heading("About");
                ui.add_space(10.0);
                ui.label("OrbWeaver / Graphchan");
                ui.label("A decentralized peer-to-peer discussion platform");
                ui.add_space(5.0);
                ui.hyperlink_to("View on GitHub", "https://github.com/yourusername/orbweaver");
            });
        });

        ui.add_space(20.0);

        ui.horizontal(|ui| {
            if ui.button("← Back to Catalog").clicked() {
                self.view = ViewState::Catalog;
            }
        });
    }
}
