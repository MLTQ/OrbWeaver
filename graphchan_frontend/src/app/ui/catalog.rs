use eframe::egui::{self, Color32, RichText};

use crate::models::ThreadSummary;

use super::super::{format_timestamp, GraphchanApp};

impl GraphchanApp {
    pub(crate) fn render_catalog(&mut self, ui: &mut egui::Ui) {
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
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.threads.is_empty() && !self.threads_loading {
                ui.label("No threads yet. Create one to get started.");
            }

            let mut thread_to_open: Option<ThreadSummary> = None;

            for thread in &self.threads {
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .inner_margin(egui::vec2(12.0, 8.0))
                    .show(ui, |ui| {
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
            }

            if let Some(thread) = thread_to_open {
                self.open_thread(thread);
            }
        });
    }
}
