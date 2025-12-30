use eframe::egui::{self, Color32, RichText};

use crate::models::ThreadSummary;

use super::super::{format_timestamp, GraphchanApp};

impl GraphchanApp {
    pub(crate) fn render_catalog(&mut self, ui: &mut egui::Ui) {
        // Toggle to show/hide ignored threads
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_ignored_threads, "Show ignored threads");
        });
        ui.add_space(10.0);

        // Partition threads based on sync_status
        let (my_threads, network_threads): (Vec<ThreadSummary>, Vec<ThreadSummary>) =
            self.threads.iter().cloned().partition(|t| {
                t.sync_status == "downloaded"
            });

        self.render_catalog_three_columns(
            ui,
            "My Threads",
            &my_threads,
            "Network Threads",
            &network_threads,
            "Recent Posts",
        );
    }

    pub(crate) fn render_friend_catalog(&mut self, ui: &mut egui::Ui, peer: &crate::models::PeerView) {
        let peer_name = peer.username.as_deref().unwrap_or("Peer");

        // Toggle to show/hide ignored threads
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.show_ignored_threads, "Show ignored threads");
        });
        ui.add_space(10.0);

        let (authored_threads, other_threads): (Vec<ThreadSummary>, Vec<ThreadSummary>) = self.threads.iter().cloned().partition(|t| {
            t.creator_peer_id.as_ref() == Some(&peer.id)
        });

        self.render_catalog_split(
            ui,
            &format!("Authored by {}", peer_name),
            &authored_threads,
            "Network Threads",
            &other_threads
        );
    }

    fn render_catalog_three_columns(
        &mut self,
        ui: &mut egui::Ui,
        title1: &str,
        threads1: &[ThreadSummary],
        title2: &str,
        threads2: &[ThreadSummary],
        title3: &str,
    ) {
        if self.threads_loading && self.threads.is_empty() {
            ui.add(egui::Spinner::new());
        }
        if let Some(err) = &self.threads_error {
            ui.colored_label(Color32::LIGHT_RED, err);
            if ui.button("Retry").clicked() {
                self.spawn_load_threads();
            }
            ui.separator();
        }

        ui.columns(3, |columns| {
            // Column 1: My Threads (all downloaded threads)
            columns[0].vertical(|ui| {
                ui.heading(title1);
                ui.add_space(10.0);
                self.render_thread_list(ui, threads1);
            });

            // Column 2: Network Threads (announced but not downloaded)
            columns[1].vertical(|ui| {
                ui.heading(title2);
                ui.add_space(10.0);
                if threads2.is_empty() {
                    ui.label(RichText::new("No network threads available").italics().weak());
                    ui.add_space(5.0);
                    ui.label("Thread announcements from peers will appear here.");
                } else {
                    self.render_thread_list(ui, threads2);
                }
            });

            // Column 3: Recent Posts
            columns[2].vertical(|ui| {
                ui.heading(title3);
                ui.add_space(10.0);
                self.render_recent_posts(ui);
            });
        });
    }

    fn render_catalog_split(
        &mut self,
        ui: &mut egui::Ui,
        title1: &str,
        threads1: &[ThreadSummary],
        title2: &str,
        threads2: &[ThreadSummary]
    ) {
        if self.threads_loading && self.threads.is_empty() {
            ui.add(egui::Spinner::new());
        }
        if let Some(err) = &self.threads_error {
            ui.colored_label(Color32::LIGHT_RED, err);
            if ui.button("Retry").clicked() {
                self.spawn_load_threads();
            }
            ui.separator();
        }

        ui.columns(2, |columns| {
            // Column 1
            columns[0].vertical(|ui| {
                ui.heading(title1);
                ui.add_space(10.0);
                self.render_thread_list(ui, threads1);
            });

            // Column 2
            columns[1].vertical(|ui| {
                ui.heading(title2);
                ui.add_space(10.0);
                self.render_thread_list(ui, threads2);
            });
        });
    }

    fn render_thread_list(&mut self, ui: &mut egui::Ui, threads: &[ThreadSummary]) {
        if threads.is_empty() {
            ui.label("No threads found.");
            return;
        }

        let mut thread_to_open: Option<ThreadSummary> = None;
        let mut thread_to_delete: Option<String> = None;
        let mut thread_to_ignore: Option<String> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for thread in threads {
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .inner_margin(egui::vec2(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.horizontal(|ui| {
                            // Show thumbnail if first image exists
                            if let Some(file) = &thread.first_image_file {
                                if let Some(texture) = self.image_textures.get(&file.id) {
                                    ui.image((texture.id(), egui::vec2(48.0, 48.0)));
                                } else if !self.image_loading.contains(&file.id) {
                                    // Queue image for loading
                                    if let Some(url) = &file.download_url {
                                        let file_id = file.id.clone();
                                        let url = url.clone();
                                        self.image_loading.insert(file_id.clone());
                                        super::super::tasks::download_image(
                                            self.tx.clone(),
                                            file_id,
                                            url,
                                        );
                                    }
                                }
                            }

                            ui.vertical(|ui| {
                                let title = if thread.title.is_empty() {
                                    "(untitled thread)"
                                } else {
                                    &thread.title
                                };
                                if ui.button(RichText::new(title).strong()).clicked() {
                                    thread_to_open = Some(thread.clone());
                                }
                                // Show creator with username lookup
                                if let Some(peer_id) = &thread.creator_peer_id {
                                    let display_name = self.peers.get(peer_id)
                                        .and_then(|p| p.username.as_deref())
                                        .unwrap_or("Anonymous");
                                    ui.label(RichText::new(format!("Created by {}", display_name)).size(11.0).weak());
                                }
                            });

                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    // Delete button (for own threads)
                                    if ui.small_button("🗑 Delete").clicked() {
                                        thread_to_delete = Some(thread.id.clone());
                                    }
                                    // Ignore button (for others' threads)
                                    if ui.small_button("🔇 Ignore").clicked() {
                                        thread_to_ignore = Some(thread.id.clone());
                                    }
                                    ui.label(format_timestamp(&thread.created_at));
                                },
                            );
                        });
                    });
                ui.add_space(8.0);
            }
        });

        if let Some(thread) = thread_to_open {
            self.open_thread(thread);
        }

        if let Some(thread_id) = thread_to_delete {
            self.spawn_delete_thread(thread_id);
        }

        if let Some(thread_id) = thread_to_ignore {
            self.spawn_ignore_thread(thread_id);
        }
    }

    fn render_recent_posts(&mut self, ui: &mut egui::Ui) {
        // Show loading/error states
        if self.recent_posts_loading && self.recent_posts.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.spinner();
            });
            return;
        }

        if let Some(error) = &self.recent_posts_error {
            ui.colored_label(Color32::RED, format!("Error: {}", error));
            return;
        }

        if self.recent_posts.is_empty() {
            ui.centered_and_justified(|ui| {
                ui.label(RichText::new("No recent posts yet").italics().weak());
            });
            return;
        }

        let mut thread_to_open: Option<String> = None;

        egui::ScrollArea::vertical().show(ui, |ui| {
            for recent_post in &self.recent_posts.clone() {
                let post = &recent_post.post;
                let thread_title = &recent_post.thread_title;

                ui.group(|ui| {
                    ui.set_width(ui.available_width());

                    // Thread title context
                    ui.label(RichText::new(thread_title).strong().size(12.0));

                    ui.add_space(4.0);

                    // Post preview - truncate to ~200 characters
                    let preview = if post.body.len() > 200 {
                        format!("{}...", &post.body[..200])
                    } else {
                        post.body.clone()
                    };
                    ui.label(RichText::new(preview).size(11.0));

                    // Show image thumbnails if available
                    if !recent_post.files.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            for file in &recent_post.files {
                                if let Some(mime) = &file.mime {
                                    if mime.starts_with("image/") {
                                        // Show image thumbnail (32x32)
                                        if let Some(texture) = self.image_textures.get(&file.id) {
                                            ui.image((texture.id(), egui::vec2(32.0, 32.0)));
                                        } else if !self.image_loading.contains(&file.id) {
                                            // Queue image for loading
                                            if let Some(url) = &file.download_url {
                                                let file_id = file.id.clone();
                                                let url = url.clone();
                                                self.image_loading.insert(file_id.clone());
                                                super::super::tasks::download_image(
                                                    self.tx.clone(),
                                                    file_id,
                                                    url,
                                                );
                                            }
                                        }
                                    }
                                }
                            }
                        });
                    }

                    ui.add_space(4.0);

                    // Timestamp and click to navigate
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format_timestamp(&post.created_at)).size(10.0).weak());

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.small_button("View →").clicked() {
                                thread_to_open = Some(post.thread_id.clone());
                            }
                        });
                    });
                });

                ui.add_space(6.0);
            }
        });

        // Handle thread opening after UI loop
        if let Some(thread_id) = thread_to_open {
            // Find the thread summary in our threads list
            if let Some(summary) = self.threads.iter().find(|t| t.id == thread_id).cloned() {
                self.open_thread(summary);
            } else {
                // Thread not in list yet, try to load it
                self.spawn_load_thread(&thread_id);
            }
        }
    }
}
