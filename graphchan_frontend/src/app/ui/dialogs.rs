use eframe::egui::{self, Align2, Color32, Context};

use super::super::state::{CreateThreadState, ImportPlatform};

use super::super::{tasks, GraphchanApp};

impl GraphchanApp {
    pub(crate) fn render_create_thread_dialog(&mut self, ctx: &Context) {
        if !self.show_create_thread {
            return;
        }

        let mut should_close = false;
        let mut should_submit = false;

        egui::Window::new("Create Thread")
            .open(&mut self.show_create_thread)
            .default_width(400.0)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                if let Some(err) = &self.create_thread.error {
                    ui.colored_label(Color32::LIGHT_RED, err);
                }
                ui.label("Title");
                ui.text_edit_singleline(&mut self.create_thread.title);
                ui.add_space(6.0);
                ui.label("Body (optional)");
                ui.add(
                    egui::TextEdit::multiline(&mut self.create_thread.body)
                        .desired_rows(6)
                        .hint_text("Write the opening post..."),
                );
                ui.add_space(6.0);
                
                if !self.create_thread.files.is_empty() {
                    ui.label("Attachments:");
                    for file in &self.create_thread.files {
                         ui.label(file.file_name().unwrap_or_default().to_string_lossy());
                    }
                    ui.add_space(6.0);
                }
                
                if ui.button("Attach Files").clicked() {
                    tasks::pick_files(self.tx.clone());
                }

                ui.add_space(8.0);

                // Topic selector
                ui.group(|ui| {
                    ui.label("📡 Announce to Topics");
                    ui.label("Select which topics to announce this thread on:");
                    ui.add_space(4.0);

                    if self.subscribed_topics.is_empty() {
                        ui.colored_label(Color32::GRAY, "No topics subscribed.");
                        ui.horizontal(|ui| {
                            ui.label("Subscribe to topics in the");
                            if ui.button("Topic Manager").clicked() {
                                self.show_topic_manager = true;
                            }
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(120.0)
                            .show(ui, |ui| {
                                for topic_id in &self.subscribed_topics.clone() {
                                    let mut is_selected = self.selected_topics.contains(topic_id);
                                    if ui.checkbox(&mut is_selected, topic_id).clicked() {
                                        if is_selected {
                                            self.selected_topics.insert(topic_id.clone());
                                        } else {
                                            self.selected_topics.remove(topic_id);
                                        }
                                    }
                                }
                            });
                    }

                    ui.add_space(4.0);
                    if self.selected_topics.is_empty() {
                        ui.colored_label(Color32::YELLOW, "⚠ No topics selected - thread will be friends-only");
                    } else {
                        ui.label(format!("✓ Will announce to {} topic(s)", self.selected_topics.len()));
                    }
                });

                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if self.create_thread.submitting {
                        ui.add(egui::Spinner::new());
                    } else if ui.button("Create").clicked() {
                        should_submit = true;
                    }
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_submit {
            self.spawn_create_thread();
        }
        if should_close {
            self.show_create_thread = false;
            self.create_thread = CreateThreadState::default();
            self.selected_topics.clear();
        }
    }

    pub(crate) fn render_import_dialog(&mut self, ctx: &Context) {
        if !self.importer.open {
            return;
        }

        let mut should_import = false;
        let mut should_close = false;

        egui::Window::new("Import Thread")
            .open(&mut self.importer.open)
            .resizable(false)
            .default_width(420.0)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                // Platform selector
                ui.horizontal(|ui| {
                    ui.label("Platform:");
                    ui.selectable_value(&mut self.importer.platform, ImportPlatform::FourChan, "4chan");
                    ui.selectable_value(&mut self.importer.platform, ImportPlatform::Reddit, "Reddit");
                });

                ui.add_space(4.0);

                let hint = match self.importer.platform {
                    ImportPlatform::FourChan => "https://boards.4chan.org/g/thread/123456789",
                    ImportPlatform::Reddit => "https://www.reddit.com/r/sub/comments/abc123/title",
                };
                ui.label(format!("Paste a thread URL (e.g. {hint})"));
                ui.text_edit_singleline(&mut self.importer.url);
                if let Some(err) = &self.importer.error {
                    ui.colored_label(Color32::LIGHT_RED, err);
                }

                ui.add_space(8.0);

                // Topic selector (same pattern as create thread dialog)
                ui.group(|ui| {
                    ui.label("Announce to Topics");
                    ui.label("Select which topics to announce this import on:");
                    ui.add_space(4.0);

                    if self.subscribed_topics.is_empty() {
                        ui.colored_label(Color32::GRAY, "No topics subscribed.");
                        ui.horizontal(|ui| {
                            ui.label("Subscribe to topics in the");
                            if ui.button("Topic Manager").clicked() {
                                self.show_topic_manager = true;
                            }
                        });
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(100.0)
                            .show(ui, |ui| {
                                for topic_id in &self.subscribed_topics.clone() {
                                    let mut is_selected = self.importer.selected_topics.contains(topic_id);
                                    if ui.checkbox(&mut is_selected, topic_id).clicked() {
                                        if is_selected {
                                            self.importer.selected_topics.insert(topic_id.clone());
                                        } else {
                                            self.importer.selected_topics.remove(topic_id);
                                        }
                                    }
                                }
                            });
                    }

                    ui.add_space(4.0);
                    if self.importer.selected_topics.is_empty() {
                        ui.colored_label(Color32::YELLOW, "No topics selected - thread will be friends-only");
                    } else {
                        ui.label(format!("Will announce to {} topic(s)", self.importer.selected_topics.len()));
                    }
                });

                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if self.importer.importing {
                        ui.add(egui::Spinner::new());
                    } else if ui.button("Import").clicked() {
                        should_import = true;
                    }
                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                });
            });

        if should_import {
            match self.importer.platform {
                ImportPlatform::FourChan => self.spawn_import_fourchan(),
                ImportPlatform::Reddit => self.spawn_import_reddit(),
            }
        }
        if should_close {
            self.importer.open = false;
            self.importer.selected_topics.clear();
        }
    }
}
