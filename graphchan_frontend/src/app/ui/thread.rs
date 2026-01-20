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

pub fn render_post_body(ui: &mut egui::Ui, body: &str) -> Option<String> {
    let mut clicked_thread = None;
    
    // Tighten line spacing to match 4chan's compact text rendering
    let original_spacing = ui.spacing().item_spacing.y;
    ui.spacing_mut().item_spacing.y = 2.0; // Tight line spacing
    
    for line in body.lines() {
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            
            let mut current_text = String::new();
            
            for word in line.split(' ') {
                if word.starts_with(">>>") {
                    // Flush current text
                    if !current_text.is_empty() {
                        ui.label(current_text.clone());
                        current_text.clear();
                    }
                    
                    // Render link
                    let thread_id = &word[3..];
                    if ui.link(word).clicked() {
                        clicked_thread = Some(thread_id.to_string());
                    }
                    ui.label(" "); // Space after link
                } else {
                    current_text.push_str(word);
                    current_text.push(' ');
                }
            }
            
            // Flush remaining text
            if !current_text.is_empty() {
                ui.label(current_text);
            }
        });
    }
    
    // Restore original spacing
    ui.spacing_mut().item_spacing.y = original_spacing;
    
    clicked_thread
}

impl GraphchanApp {
    pub(crate) fn open_thread(&mut self, summary: ThreadSummary) {
        let thread_id = summary.id.clone();
        self.view = ViewState::Thread(ThreadState {
            summary,
            is_loading: true,
            graph_zoom: 1.0,
            time_bin_seconds: 60,
            repulsion_force: 500.0,
            desired_edge_length: 1.5,
            is_hosting: true, // Default to Host mode
            ..Default::default()
        });
        self.spawn_load_thread(&thread_id);
    }

    fn render_reactions(&mut self, ui: &mut egui::Ui, state: &mut ThreadState, post_id: &str) {
        // Load reactions if not already loaded/loading
        if !state.reactions.contains_key(post_id) && !state.reactions_loading.contains(post_id) {
            state.reactions_loading.insert(post_id.to_string());
            self.spawn_load_reactions(post_id);
        }

        ui.horizontal_wrapped(|ui| {
            // Show existing reactions
            if let Some(reactions_response) = state.reactions.get(post_id) {
                for (emoji, count) in &reactions_response.counts {
                    let button_text = format!("{} {}", emoji, count);
                    if ui.small_button(button_text).clicked() {
                        // Toggle reaction: remove if user already reacted, otherwise add
                        let self_peer_id = self.identity_state.local_peer.as_ref().map(|p| p.id.clone());
                        if let Some(self_id) = self_peer_id {
                            let has_reacted = reactions_response.reactions.iter()
                                .any(|r| r.emoji == *emoji && r.reactor_peer_id == self_id);

                            if has_reacted {
                                self.spawn_remove_reaction(post_id, emoji);
                            } else {
                                self.spawn_add_reaction(post_id, emoji);
                            }
                        }
                    }
                }
            }

            // "+" button to open emoji picker
            let picker_open = state.emoji_picker_open.as_ref() == Some(&post_id.to_string());
            if ui.small_button(if picker_open { "−" } else { "+" }).clicked() {
                if picker_open {
                    state.emoji_picker_open = None;
                } else {
                    state.emoji_picker_open = Some(post_id.to_string());
                }
            }

            // Show emoji picker if open for this post
            if picker_open {
                ui.separator();
                ui.label("Quick reactions:");
                let quick_emojis = ["👍", "❤️", "😂", "🎉", "🤔", "👀", "🔥", "✨"];
                for emoji in quick_emojis {
                    if ui.small_button(emoji).clicked() {
                        self.spawn_add_reaction(post_id, emoji);
                        state.emoji_picker_open = None;
                    }
                }
            }

            // Show loading indicator
            if state.reactions_loading.contains(post_id) {
                ui.spinner();
            }
        });
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

            // Host/Leech toggle on the right
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let button_text = if state.is_hosting { "Host" } else { "Leech" };
                let button_color = if state.is_hosting {
                    Color32::from_rgb(100, 150, 100) // Subtle green for Host
                } else {
                    Color32::from_rgb(150, 80, 80) // Dull red for Leech
                };

                let button = egui::Button::new(RichText::new(button_text).color(button_color))
                    .frame(true);

                if ui.add(button).clicked() {
                    state.is_hosting = !state.is_hosting;
                }

                // Tooltip explanation
                if state.is_hosting {
                    ui.label(RichText::new("Broadcasting thread to peers").small().color(Color32::GRAY));
                } else {
                    ui.label(RichText::new("Not broadcasting (lurking)").small().color(Color32::GRAY));
                }
            });
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
            ui.selectable_value(&mut state.display_mode, ThreadDisplayMode::List, "List");
            ui.selectable_value(&mut state.display_mode, ThreadDisplayMode::Graph, "Graph");
            ui.selectable_value(
                &mut state.display_mode,
                ThreadDisplayMode::Chronological,
                "Timeline",
            );
            ui.selectable_value(&mut state.display_mode, ThreadDisplayMode::Sugiyama, "Hierarchical");
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
        } else if state.display_mode == ThreadDisplayMode::Sugiyama {
            use super::sugiyama;
            sugiyama::render_sugiyama(self, ui, state);
        } else if state.display_mode == ThreadDisplayMode::List {
            if let Some(details) = &state.details {
            let posts_clone = details.posts.clone();
            let thread_id = state.summary.id.clone();
            let attachments = &state.attachments;
            let attachments_loading = &state.attachments_loading;
            let attachments_errors = &state.attachments_errors;
            let api_base = self.api.base_url().to_string();

            // Calculate max reaction score for normalization
            let max_score = super::reaction_colors::calculate_max_score_from_responses(&state.reactions);

            // Build children map for reply links
            let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
            for post in &posts_clone {
                for parent_id in &post.parent_post_ids {
                    children_map.entry(parent_id.clone()).or_default().push(post.id.clone());
                }
            }

            egui::ScrollArea::vertical()
                .id_source("thread-posts")
                .show(ui, |ui| {
                    for post in &posts_clone {
                        // Check if we need to scroll to this post
                        let should_scroll = state.scroll_to_post.as_ref() == Some(&post.id);

                        // Calculate reaction-based border color
                        let border_color = if let Some(reactions_response) = state.reactions.get(&post.id) {
                            super::reaction_colors::calculate_post_color(&reactions_response.counts, max_score)
                        } else {
                            Color32::from_rgb(80, 90, 130) // Default
                        };

                        let frame = egui::Frame::group(ui.style())
                            .fill(ui.visuals().extreme_bg_color)
                            .inner_margin(egui::vec2(12.0, 8.0))
                            .stroke(egui::Stroke::new(2.0, border_color));

                        let response = frame.show(ui, |ui| {
                            // Scroll to this post if needed
                            if should_scroll {
                                ui.scroll_to_cursor(Some(egui::Align::Center));
                            }
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

                                        // Display agent badge if post has agent metadata
                                        if let Some(metadata) = &post.metadata {
                                            if let Some(agent) = &metadata.agent {
                                                ui.add_space(4.0);
                                                let badge_text = if let Some(version) = &agent.version {
                                                    format!("[Agent: {} v{}]", agent.name, version)
                                                } else {
                                                    format!("[Agent: {}]", agent.name)
                                                };
                                                ui.label(
                                                    RichText::new(badge_text)
                                                        .size(10.0)
                                                        .color(Color32::from_rgb(150, 200, 255))
                                                );
                                            }
                                        }
                                    });
                                }
                                ui.separator();

                                // Parent links (top of post)
                                if !post.parent_post_ids.is_empty() {
                                    ui.add_space(4.0);
                                    ui.horizontal_wrapped(|ui| {
                                        ui.label(RichText::new("↩ Replying to:").color(Color32::GRAY));
                                        for parent_id in &post.parent_post_ids {
                                            let short_id = if parent_id.len() > 8 { &parent_id[..8] } else { parent_id };
                                            if ui.link(RichText::new(format!(">>{}", short_id)).size(11.0)).clicked() {
                                                state.selected_post = Some(parent_id.clone());
                                                state.scroll_to_post = Some(parent_id.clone());
                                            }
                                        }
                                    });
                                }

                                ui.add_space(6.0);
                                if let Some(thread_id) = render_post_body(ui, &post.body) {
                                    action = ThreadAction::OpenThread(thread_id);
                                }

                                if !post.files.is_empty() {
                                    ui.label(RichText::new("Attachments").strong());
                                    for file in &post.files {
                                        self.render_file_attachment(ui, file, &api_base);
                                    }
                                }

                                // Reply links (bottom of post)
                                if let Some(children) = children_map.get(&post.id) {
                                    if !children.is_empty() {
                                        ui.add_space(4.0);
                                        ui.horizontal_wrapped(|ui| {
                                            ui.label(RichText::new("↪ Replies:").color(Color32::GRAY));
                                            for child_id in children {
                                                let short_id = if child_id.len() > 8 { &child_id[..8] } else { child_id };
                                                if ui.link(RichText::new(format!(">>{}", short_id)).size(11.0)).clicked() {
                                                    state.selected_post = Some(child_id.clone());
                                                    state.scroll_to_post = Some(child_id.clone());
                                                }
                                            }
                                        });
                                    }
                                }

                                // Reactions UI
                                ui.add_space(6.0);
                                self.render_reactions(ui, state, &post.id);
                            });

                        // Clear scroll flag after frame is shown
                        if should_scroll {
                            state.scroll_to_post = None;
                        }
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
