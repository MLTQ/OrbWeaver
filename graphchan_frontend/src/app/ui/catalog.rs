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

        // All threads in self.threads are locally downloaded
        let my_threads = self.threads.clone();

        // Network threads would come from thread announcements not yet downloaded
        // For now, this is empty - requires backend support
        let network_threads: Vec<ThreadSummary> = Vec::new();

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
                            let title = if thread.title.is_empty() {
                                "(untitled thread)"
                            } else {
                                &thread.title
                            };
                            if ui.button(RichText::new(title).strong()).clicked() {
                                thread_to_open = Some(thread.clone());
                            }
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
                                    ui.label(RichText::new(&thread.id).monospace().size(10.0));
                                },
                            );
                        });
                        if let Some(peer) = &thread.creator_peer_id {
                            ui.label(format!("Created by {peer}"));
                        }
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
        // TODO: This requires backend support to stream posts or a dedicated API endpoint
        // For now, show a placeholder

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.label(RichText::new("Recent Posts feed coming soon...").italics().weak());
            ui.add_space(10.0);
            ui.label("This will show a live feed of new posts from all your subscribed threads, with:");
            ui.label("• Post previews");
            ui.label("• Image thumbnails");
            ui.label("• Thread context");
            ui.label("• Click to navigate to thread");

            ui.add_space(10.0);
            ui.separator();
            ui.label(RichText::new("Requires backend implementation").size(11.0).weak());
        });
    }
}
