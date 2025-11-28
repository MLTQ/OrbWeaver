use std::collections::{HashMap, HashSet};

use eframe::egui::{self, Color32, RichText};

use crate::models::ThreadSummary;

use super::super::state::{ThreadDisplayMode, ThreadState, ViewState};
use super::super::{format_timestamp, GraphchanApp};
use super::{chronological, graph};

pub enum ThreadAction {
    None,
    GoBack,
    OpenThread(String),
}

fn render_post_body(ui: &mut egui::Ui, body: &str) -> Option<String> {
    let mut clicked_thread = None;
    for line in body.lines() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, word) in line.split(' ').enumerate() {
                if i > 0 {
                    ui.label(" ");
                }
                if word.starts_with(">>>") {
                    let thread_id = &word[3..];
                    if ui.link(word).clicked() {
                        clicked_thread = Some(thread_id.to_string());
                    }
                } else {
                    ui.label(word);
                }
            }
        });
    }
    clicked_thread
}

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
            chronological_nodes: HashMap::new(),
            sim_start_time: None,
            selected_post: None,
            graph_zoom: 1.0,
            graph_offset: egui::vec2(0.0, 0.0),
            graph_dragging: false,
            reply_to: Vec::new(),
            time_bin_seconds: 60, // Default to 1 minute bins
            locked_hover_post: None,
            repulsion_force: 500.0,
            sim_paused: false,
            draft_attachments: Vec::new(),
        });
        self.spawn_load_thread(&thread_id);
    }

    pub(crate) fn render_thread(
        &mut self,
        ui: &mut egui::Ui,
        state: &mut ThreadState,
    ) -> ThreadAction {
        let mut action = ThreadAction::None;
        let mut should_retry = false;
        let mut should_create_post = false;

        ui.horizontal(|ui| {
            if ui.button("← Back to catalog").clicked() {
                action = ThreadAction::GoBack;
            }
            ui.separator();
            ui.label(RichText::new(&state.summary.title).heading());
        });

        if state.is_loading {
            ui.add(egui::Spinner::new());
            return action;
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
            return action;
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
                    state.chronological_nodes = HashMap::new();
                    state.sim_start_time = None;
                    state.graph_zoom = 1.0;
                    state.graph_offset = egui::vec2(0.0, 0.0);
                    state.graph_dragging = false;
                }
            }
            graph::render_graph(self, ui, state);
        } else if state.display_mode == ThreadDisplayMode::Chronological {
            if state.graph_nodes.is_empty() {
                if let Some(details) = &state.details {
                    state.graph_nodes = chronological::build_chronological_layout(&details.posts, state.time_bin_seconds);
                    state.graph_zoom = 1.0;
                    state.graph_offset = egui::vec2(0.0, 0.0);
                }
            }
            chronological::render_chronological(self, ui, state);
        } else if state.display_mode == ThreadDisplayMode::List {
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
                                    if ui.button(RichText::new(&post.id).monospace()).clicked() {
                                        Self::quote_post(state, &post.id);
                                    }
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(format_timestamp(&post.created_at));
                                        },
                                    );
                                });
                                if let Some(author) = &post.author_peer_id {
                                    let peer = self.peers.get(author).cloned();
                                    ui.horizontal(|ui| {
                                        let avatar_id = peer.as_ref().and_then(|p| p.avatar_file_id.clone());
                                        if let Some(avatar_id) = avatar_id {
                                            if let Some(texture) = self.image_textures.get(&avatar_id) {
                                                ui.add(egui::Image::from_texture(texture).max_width(32.0).rounding(16.0));
                                            } else if let Some(pending) = self.image_pending.remove(&avatar_id) {
                                                let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
                                                let tex = ui.ctx().load_texture(&avatar_id, color, egui::TextureOptions::default());
                                                self.image_textures.insert(avatar_id.clone(), tex.clone());
                                                ui.add(egui::Image::from_texture(&tex).max_width(32.0).rounding(16.0));
                                            } else if let Some(err) = self.image_errors.get(&avatar_id) {
                                                ui.colored_label(egui::Color32::RED, "Error");
                                                ui.label(err);
                                            } else {
                                                let needs_download = !self.image_loading.contains(&avatar_id);
                                                if needs_download {
                                                    let url = crate::app::resolve_blob_url(&self.base_url_input, &avatar_id);
                                                    self.spawn_download_image(&avatar_id, &url);
                                                }
                                                ui.spinner();
                                            }
                                        }
                                        
                                        let name = peer.as_ref().and_then(|p| p.username.as_deref()).unwrap_or(author);
                                        if ui.link(format!("Author: {}", name)).clicked() {
                                            self.show_identity = true;
                                            self.identity_state.inspected_peer = peer.clone();
                                        }
                                    });
                                }
                                ui.separator();
                                if let Some(thread_id) = render_post_body(ui, &post.body) {
                                    action = ThreadAction::OpenThread(thread_id);
                                }
                                if !post.parent_post_ids.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(format!(
                                        "Replies to: {}",
                                        post.parent_post_ids.join(", ")
                                    ));
                                }
                                ui.add_space(6.0);

                                if !post.files.is_empty() {
                                    ui.label(RichText::new("Attachments").strong());
                                    for file in &post.files {
                                        self.render_file_attachment(ui, file, &api_base);
                                    }
                                }
                            });


                    }
                });
        }
    }

        ui.separator();
        ui.separator();
        
        // Floating composer is handled separately
        if self.render_floating_composer(ui.ctx(), state) {
            should_create_post = true;
        }

        if should_create_post {
            self.spawn_create_post(state);
        }

        action
    }

    fn render_floating_composer(&mut self, ctx: &egui::Context, state: &mut ThreadState) -> bool {
        let mut should_create = false;
        
        egui::Window::new("Draft Post")
            .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
            .default_width(300.0)
            .collapsible(true)
            .resizable(true)
            .show(ctx, |ui| {
                if let Some(err) = &state.new_post_error {
                    ui.colored_label(Color32::LIGHT_RED, err);
                }
                
                // Reply targets
                if !state.reply_to.is_empty() {
                    ui.horizontal_wrapped(|ui| {
                        ui.label("Replying to:");
                        let mut to_remove = None;
                        for (idx, id) in state.reply_to.iter().enumerate() {
                            if ui.button(format!("❌ #{}", id)).clicked() {
                                to_remove = Some(idx);
                            }
                        }
                        if let Some(idx) = to_remove {
                            state.reply_to.remove(idx);
                        }
                        if ui.button("Clear All").clicked() {
                            state.reply_to.clear();
                        }
                    });
                    ui.separator();
                }

                // Body
                ui.add(
                    egui::TextEdit::multiline(&mut state.new_post_body)
                        .desired_rows(4)
                        .hint_text("Write a reply..."),
                );
                
                ui.separator();
                
                // Attachments
                ui.label("Attachments:");
                if !state.draft_attachments.is_empty() {
                    let mut to_remove = None;
                    for (idx, path) in state.draft_attachments.iter().enumerate() {
                        ui.horizontal(|ui| {
                            let name = path.file_name().map(|s| s.to_string_lossy()).unwrap_or_default();
                            ui.label(name);
                            if ui.button("❌").clicked() {
                                to_remove = Some(idx);
                            }
                        });
                    }
                    if let Some(idx) = to_remove {
                        state.draft_attachments.remove(idx);
                    }
                }
                
                if ui.button("📎 Add Attachment").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_file() {
                        state.draft_attachments.push(path);
                    }
                }

                ui.separator();

                // Actions
                ui.horizontal(|ui| {
                    if state.new_post_sending {
                        ui.add(egui::Spinner::new());
                        ui.label("Sending...");
                    } else {
                        if ui.button("Post").clicked() {
                            should_create = true;
                        }
                    }
                    if ui.button("Clear Draft").clicked() {
                        state.new_post_body.clear();
                        state.new_post_error = None;
                        state.reply_to.clear();
                        state.draft_attachments.clear();
                    }
                });
            });
            
        should_create
    }
}
