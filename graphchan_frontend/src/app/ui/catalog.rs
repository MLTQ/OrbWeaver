use eframe::egui::{self, Color32, RichText};

use crate::models::ThreadSummary;

use super::super::{format_timestamp, GraphchanApp};

impl GraphchanApp {
    pub(crate) fn render_catalog(&mut self, ui: &mut egui::Ui) {
        let local_peer_id = self.identity_state.local_peer.as_ref().map(|p| p.id.clone());
        
        let (my_threads, network_threads): (Vec<ThreadSummary>, Vec<ThreadSummary>) = self.threads.iter().cloned().partition(|t| {
            if let Some(local_id) = &local_peer_id {
                t.creator_peer_id.as_ref() == Some(local_id)
            } else {
                false
            }
        });

        self.render_catalog_split(ui, "My Threads", &my_threads, "Network Threads", &network_threads);
    }

    pub(crate) fn render_friend_catalog(&mut self, ui: &mut egui::Ui, peer: &crate::models::PeerView) {
        let peer_name = peer.username.as_deref().unwrap_or("Friend");
        
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
    }
}
