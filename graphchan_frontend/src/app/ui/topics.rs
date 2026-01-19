use eframe::egui::{self, Align2, Color32, Context};

use super::super::GraphchanApp;

impl GraphchanApp {
    pub(crate) fn render_topic_manager(&mut self, ctx: &Context) {
        if !self.show_topic_manager {
            return;
        }

        let mut should_close = false;
        let mut topic_to_subscribe: Option<String> = None;
        let mut topic_to_unsubscribe: Option<String> = None;

        egui::Window::new("Topic Manager")
            .open(&mut self.show_topic_manager)
            .default_width(500.0)
            .default_height(400.0)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.heading("Manage Topics");
                ui.add_space(8.0);

                // Subscribe to new topic section
                ui.group(|ui| {
                    ui.label("Subscribe to a Topic");
                    ui.horizontal(|ui| {
                        ui.label("Topic ID:");
                        ui.text_edit_singleline(&mut self.new_topic_input);
                        if ui.button("Subscribe").clicked() && !self.new_topic_input.is_empty() {
                            topic_to_subscribe = Some(self.new_topic_input.trim().to_string());
                        }
                    });
                    ui.label("Enter a topic name like \"tech\", \"art\", \"gaming\"");
                });

                ui.add_space(12.0);

                // Show error if any
                if let Some(err) = &self.topics_error {
                    ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
                    ui.add_space(8.0);
                }

                // Subscribed topics section
                ui.separator();
                ui.heading("Your Topics");
                ui.add_space(8.0);

                if self.topics_loading {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new());
                        ui.label("Loading topics...");
                    });
                } else if self.subscribed_topics.is_empty() {
                    ui.colored_label(Color32::GRAY, "No topics subscribed yet.");
                    ui.label("Subscribe to topics to discover content from other users!");
                } else {
                    egui::ScrollArea::vertical()
                        .max_height(250.0)
                        .show(ui, |ui| {
                            for topic_id in &self.subscribed_topics.clone() {
                                ui.horizontal(|ui| {
                                    ui.label(format!("📡 {}", topic_id));
                                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                        if ui.button("Unsubscribe").clicked() {
                                            topic_to_unsubscribe = Some(topic_id.clone());
                                        }
                                    });
                                });
                                ui.separator();
                            }
                        });

                    ui.add_space(8.0);
                    ui.label(format!("Total: {} topic(s)", self.subscribed_topics.len()));
                }

                ui.add_space(12.0);

                // Close button
                ui.horizontal(|ui| {
                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                });
            });

        // Handle actions
        if let Some(topic_id) = topic_to_subscribe {
            self.spawn_subscribe_topic(topic_id);
            self.new_topic_input.clear();
        }

        if let Some(topic_id) = topic_to_unsubscribe {
            self.spawn_unsubscribe_topic(topic_id);
        }

        if should_close {
            self.show_topic_manager = false;
        }
    }
}
