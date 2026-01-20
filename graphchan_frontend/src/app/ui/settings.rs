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

            // Theme Customization Section
            ui.group(|ui| {
                ui.heading("Theme Customization");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label("Primary Color:");
                        ui.add_space(5.0);

                        let _old_color = self.primary_color;
                        if ui.color_edit_button_srgba(&mut self.primary_color).changed() {
                            // Mark theme as dirty to reapply on next frame
                            self.theme_dirty = true;

                            // Save to backend
                            let r = self.primary_color.r();
                            let g = self.primary_color.g();
                            let b = self.primary_color.b();
                            let api = self.api.clone();
                            let _tx = self.tx.clone();
                            std::thread::spawn(move || {
                                let _ = api.set_theme_color(r, g, b);
                            });
                        }
                    });

                    ui.add_space(20.0);

                    // Show generated color scheme
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("Generated Color Scheme:").strong());
                        ui.add_space(5.0);

                        let scheme = crate::color_theme::ColorScheme::from_primary(self.primary_color);

                        ui.horizontal(|ui| {
                            // Show colors from darkest to brightest (like the examples)

                            // Background (quaternary - darkest)
                            ui.vertical(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(60.0, 60.0),
                                    egui::Sense::hover()
                                );
                                ui.painter().rect_filled(rect, 4.0, scheme.quaternary);
                                ui.label(egui::RichText::new("BG").small());
                            });

                            ui.add_space(8.0);

                            // Card (tertiary - dark)
                            ui.vertical(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(60.0, 60.0),
                                    egui::Sense::hover()
                                );
                                ui.painter().rect_filled(rect, 4.0, scheme.tertiary);
                                ui.label(egui::RichText::new("Card").small());
                            });

                            ui.add_space(8.0);

                            // Accent (secondary - medium)
                            ui.vertical(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(60.0, 60.0),
                                    egui::Sense::hover()
                                );
                                ui.painter().rect_filled(rect, 4.0, scheme.secondary);
                                ui.label(egui::RichText::new("Accent").small());
                            });

                            ui.add_space(8.0);

                            // Primary (brightest)
                            ui.vertical(|ui| {
                                let (rect, _) = ui.allocate_exact_size(
                                    egui::vec2(60.0, 60.0),
                                    egui::Sense::hover()
                                );
                                ui.painter().rect_filled(rect, 4.0, scheme.primary);
                                ui.label(egui::RichText::new("Primary").small());
                            });
                        });
                    });
                });

                ui.add_space(10.0);

                if ui.button("Reset to Default Blue").clicked() {
                    self.primary_color = egui::Color32::from_rgb(64, 128, 255);
                    self.theme_dirty = true;

                    let api = self.api.clone();
                    std::thread::spawn(move || {
                        let _ = api.set_theme_color(64, 128, 255);
                    });
                }

                ui.add_space(5.0);
                ui.label(egui::RichText::new("Theme will update immediately when you change the color").small().color(egui::Color32::GRAY));
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
