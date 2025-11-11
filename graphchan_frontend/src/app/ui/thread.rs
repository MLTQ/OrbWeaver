use std::collections::{HashMap, HashSet};

use eframe::egui::{self, Color32, RichText};

use crate::models::ThreadSummary;

use super::super::state::{ThreadDisplayMode, ThreadState, ViewState};
use super::super::{format_timestamp, GraphchanApp};
use super::{chronological, graph};

impl GraphchanApp {
    pub(crate) fn open_thread(&mut self, summary: ThreadSummary) {
        let thread_id = summary.id.clone();
        self.view = ViewState::Thread(ThreadState {
            summary,
            details: None,
            is_loading: true,
            error: None,
            new_post_body: String::new(),
            new_post_error: None,
            new_post_sending: false,
            attachments: HashMap::new(),
            attachments_loading: HashSet::new(),
            attachments_errors: HashMap::new(),
            display_mode: ThreadDisplayMode::List,
            last_layout_mode: None,
            graph_nodes: HashMap::new(),
            selected_post: None,
            graph_zoom: 1.0,
            graph_offset: egui::vec2(0.0, 0.0),
            graph_dragging: false,
            reply_to: Vec::new(),
        });
        self.spawn_load_thread(&thread_id);
    }

    pub(crate) fn render_thread_view(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut ThreadState,
    ) -> bool {
        let mut go_back = false;
        let mut should_retry = false;
        let mut should_create_post = false;

        ui.horizontal(|ui| {
            if ui.button("← Back to catalog").clicked() {
                go_back = true;
            }
            ui.separator();
            ui.label(RichText::new(&state.summary.title).heading());
        });

        if state.is_loading {
            ui.add(egui::Spinner::new());
            return go_back;
        }

        if let Some(err) = &state.error {
            ui.colored_label(Color32::LIGHT_RED, err);
            if ui.button("Retry").clicked() {
                should_retry = true;
            }
            if should_retry {
                state.is_loading = true;
                state.error = None;
                let thread_id = state.summary.id.clone();
                self.spawn_load_thread(&thread_id);
            }
            return go_back;
        }

        ui.horizontal(|ui| {
            ui.selectable_value(&mut state.display_mode, ThreadDisplayMode::List, "Posts");
            ui.selectable_value(&mut state.display_mode, ThreadDisplayMode::Graph, "Graph");
            ui.selectable_value(
                &mut state.display_mode,
                ThreadDisplayMode::Chronological,
                "Timeline",
            );
        });

        // Clear layout if mode changed
        if state.last_layout_mode != Some(state.display_mode) {
            state.graph_nodes.clear();
            state.last_layout_mode = Some(state.display_mode);
        }

        if state.display_mode == ThreadDisplayMode::Graph {
            if state.graph_nodes.is_empty() {
                if let Some(details) = &state.details {
                    state.graph_nodes = graph::build_initial_graph(&details.posts);
                    state.graph_zoom = 1.0;
                    state.graph_offset = egui::vec2(0.0, 0.0);
                    state.graph_dragging = false;
                }
            }
            graph::render_graph(self, ui, state);
            return go_back;
        }

        if state.display_mode == ThreadDisplayMode::Chronological {
            if state.graph_nodes.is_empty() {
                if let Some(details) = &state.details {
                    state.graph_nodes = chronological::build_chronological_layout(&details.posts);
                    state.graph_zoom = 1.0;
                    state.graph_offset = egui::vec2(0.0, 0.0);
                }
            }
            chronological::render_chronological(self, ui, state);
            return go_back;
        }

        if let Some(details) = &state.details {
            let posts_clone = details.posts.clone();
            let thread_id = state.summary.id.clone();
            let attachments = &state.attachments;
            let attachments_loading = &state.attachments_loading;
            let attachments_errors = &state.attachments_errors;
            let api_base = self.api.base_url().to_string();

            egui::ScrollArea::vertical()
                .id_source("thread-posts")
                .show(ui, |ui| {
                    for post in &posts_clone {
                        egui::Frame::group(ui.style())
                            .fill(ui.visuals().extreme_bg_color)
                            .inner_margin(egui::vec2(12.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&post.id).monospace());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(format_timestamp(&post.created_at));
                                        },
                                    );
                                });
                                if let Some(author) = &post.author_peer_id {
                                    ui.label(format!("Author: {author}"));
                                }
                                ui.separator();
                                ui.label(&post.body);
                                if !post.parent_post_ids.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(format!(
                                        "Replies to: {}",
                                        post.parent_post_ids.join(", ")
                                    ));
                                }
                                ui.add_space(6.0);

                                if let Some(err) = attachments_errors.get(&post.id) {
                                    ui.colored_label(Color32::LIGHT_RED, err);
                                } else if attachments_loading.contains(&post.id) {
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Spinner::new());
                                        ui.label("Loading attachments…");
                                    });
                                } else if let Some(files) = attachments.get(&post.id) {
                                    if !files.is_empty() {
                                        ui.label(RichText::new("Attachments").strong());
                                        for file in files {
                                            self.render_file_attachment(ui, file, &api_base);
                                        }
                                    }
                                }
                            });

                        if !state.attachments.contains_key(&post.id)
                            && !state.attachments_loading.contains(&post.id)
                            && !state.attachments_errors.contains_key(&post.id)
                        {
                            self.spawn_load_attachments_for_post(&thread_id, &post.id);
                        }
                    }
                });
        }

        ui.separator();
        ui.collapsing("Reply", |ui| {
            if let Some(err) = &state.new_post_error {
                ui.colored_label(Color32::LIGHT_RED, err);
            }
            if !state.reply_to.is_empty() {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!(
                        "Replying to {}",
                        state
                            .reply_to
                            .iter()
                            .map(|id| format!("#{id}"))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )));
                    if ui.button("Clear").clicked() {
                        state.reply_to.clear();
                    }
                });
            }
            ui.add(
                egui::TextEdit::multiline(&mut state.new_post_body)
                    .desired_rows(4)
                    .hint_text("Write a reply..."),
            );
            ui.horizontal(|ui| {
                if state.new_post_sending {
                    ui.add(egui::Spinner::new());
                } else if ui.button("Post").clicked() {
                    should_create_post = true;
                }
                if ui.button("Cancel").clicked() {
                    state.new_post_body.clear();
                    state.new_post_error = None;
                }
            });
        });

        if should_create_post {
            self.spawn_create_post(state);
        }

        go_back
    }
}
