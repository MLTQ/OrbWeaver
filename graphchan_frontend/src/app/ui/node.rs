use eframe::egui::{self, Color32, Margin, RichText};
use crate::models::{FileResponse, PostView};
use super::super::state::{ThreadState, ViewState};
use super::super::{format_timestamp, GraphchanApp};
use super::thread::render_post_body;


pub struct NodeLayoutData {
    pub post: PostView,
    pub rect: egui::Rect,
    pub attachments: Option<Vec<FileResponse>>,
}

pub enum ImagePreview {
    Ready(egui::TextureHandle),
    Loading,
    Error(String),
    None,
}

pub fn image_preview(
    app: &mut GraphchanApp,
    ui: &egui::Ui,
    file: &FileResponse,
    api_base: &str,
) -> ImagePreview {
    if let Some(err) = app.image_errors.get(&file.id) {
        return ImagePreview::Error(err.clone());
    }
    if let Some(tex) = app.image_textures.get(&file.id) {
        return ImagePreview::Ready(tex.clone());
    }
    if let Some(pending) = app.image_pending.remove(&file.id) {
        let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
        let tex = ui.ctx().load_texture(&file.id, color, egui::TextureOptions::default());
        app.image_textures.insert(file.id.clone(), tex.clone());
        return ImagePreview::Ready(tex);
    }
    if !app.image_loading.contains(&file.id) {
        let url = crate::app::resolve_download_url(api_base, file.download_url.as_deref(), &file.id);
        app.spawn_download_image(&file.id, &url);
    }
    ImagePreview::Loading
}

pub fn render_node(
    app: &mut GraphchanApp,
    ui: &mut egui::Ui,
    state: &mut ThreadState,
    layout: &NodeLayoutData,
    api_base: &str,
    viewport: egui::Rect,
    zoom: f32,
    children: &[String],
    is_neighbor: bool,
    is_secondary: bool,
) -> egui::Response {
    let rect_node = layout.rect;
    let selected = state.selected_post.as_ref() == Some(&layout.post.id);
    let reply_target = state.reply_to.iter().any(|id| id == &layout.post.id);

    let fill_color = if reply_target {
        Color32::from_rgb(40, 52, 85)
    } else if selected {
        Color32::from_rgb(58, 48, 24)
    } else if is_secondary {
        Color32::from_rgb(35, 35, 45) // Slightly lighter background for secondary
    } else {
        Color32::from_rgb(30, 30, 38)
    };

    // Calculate reaction-based color
    let reaction_color = if let Some(reactions_response) = state.reactions.get(&layout.post.id) {
        let max_score = super::reaction_colors::calculate_max_score_from_responses(&state.reactions);
        super::reaction_colors::calculate_post_color(&reactions_response.counts, max_score)
    } else {
        Color32::from_rgb(80, 90, 130) // Default
    };

    let stroke_color = if reply_target {
        Color32::from_rgb(255, 190, 92)
    } else if selected {
        Color32::from_rgb(250, 208, 108)
    } else if is_secondary {
        Color32::WHITE // White outline for secondary
    } else if is_neighbor {
        Color32::from_rgb(200, 180, 80) // Slightly dimmer yellow for neighbors
    } else {
        reaction_color // Use reaction-based color
    };

    // Render content at the position, letting it size naturally
    let mut content_rect = rect_node;

    let response = ui.allocate_new_ui(
        egui::UiBuilder::new()
            .max_rect(egui::Rect::from_min_size(
                rect_node.min,
                egui::vec2(rect_node.width(), f32::INFINITY)
            )),
        |ui| {
            // Scale style (fonts and spacing)
            let mut style = (**ui.style()).clone();
            for (_text_style, font_id) in style.text_styles.iter_mut() {
                font_id.size *= zoom;
            }
            style.spacing.item_spacing *= zoom;
            style.spacing.window_margin.left *= zoom;
            style.spacing.window_margin.right *= zoom;
            style.spacing.window_margin.top *= zoom;
            style.spacing.window_margin.bottom *= zoom;
            style.spacing.button_padding *= zoom;
            style.spacing.indent *= zoom;
            style.spacing.interact_size *= zoom;
            style.spacing.icon_width *= zoom;
            style.spacing.icon_width_inner *= zoom;
            style.spacing.icon_spacing *= zoom;
            ui.set_style(style);

            let inner_response = egui::Frame::none()
                .inner_margin(Margin::same(10.0 * zoom))
                .fill(fill_color)
                .stroke(egui::Stroke::new((if selected || is_neighbor || is_secondary { 2.5 } else { 1.5 }) * zoom, stroke_color))
                .rounding(egui::Rounding::same(10.0 * zoom))
                .show(ui, |ui| {
                    ui.vertical(|ui| {
                        // Top-down layout for everything
                        render_node_header(app, ui, state, &layout.post, zoom);

                        if !layout.post.parent_post_ids.is_empty() {
                            ui.add_space(4.0 * zoom);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("↩ Replying to:").size(11.0 * zoom).color(Color32::GRAY));
                                for (i, parent_id) in layout.post.parent_post_ids.iter().enumerate() {
                                    let is_parent_cursor = selected && state.parent_cursor_index == i;
                                    render_post_link(ui, state, parent_id, zoom, viewport, is_parent_cursor, false, false);
                                }
                            });
                        }

                        ui.add_space(6.0 * zoom);

                        render_post_body(ui, &layout.post.body);

                        render_node_attachments(app, ui, layout.attachments.as_ref(), api_base, zoom);

                        ui.add_space(6.0 * zoom);
                        render_node_actions(app, ui, state, &layout.post, zoom);

                        // Replies section at the bottom with proper spacing
                        if !children.is_empty() {
                            ui.add_space(6.0 * zoom);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("↪ Replies:").size(11.0 * zoom).color(Color32::GRAY));
                                for (i, child_id) in children.iter().enumerate() {
                                    let is_reply_cursor = selected && state.reply_cursor_index == i;
                                    render_post_link(ui, state, child_id, zoom, viewport, false, is_reply_cursor, true);
                                }
                            });
                        }
                    });
                });

            content_rect = inner_response.response.rect;
            inner_response.response
        }
    ).response;

    // Update actual content rect for interaction
    let naturally_hovered = ui.ctx().pointer_hover_pos()
        .map(|pos| content_rect.contains(pos))
        .unwrap_or(false);
    let locked_hover = state.locked_hover_post.as_ref() == Some(&layout.post.id);
    let _hovered = naturally_hovered || locked_hover;

    // Add background interaction layer for clicking on empty areas
    let bg_response = ui.interact(content_rect, ui.id().with(&layout.post.id).with("node_bg"), egui::Sense::click());
    if bg_response.clicked() {
        state.selected_post = Some(layout.post.id.clone());
        if state.locked_hover_post.as_ref() == Some(&layout.post.id) {
            state.locked_hover_post = None;
        } else {
            state.locked_hover_post = Some(layout.post.id.clone());
        }
    }

    response
}

fn render_node_header(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView, zoom: f32) {
    ui.horizontal(|ui| {
        if let Some(author_id) = &post.author_peer_id {
            let peer = app.peers.get(author_id).cloned();
            if let Some(avatar_id) = peer.as_ref().and_then(|p| p.avatar_file_id.clone()) {
                if let Some(texture) = app.image_textures.get(&avatar_id) {
                    ui.add(egui::Image::from_texture(texture).max_width(24.0 * zoom).rounding(12.0 * zoom));
                } else if let Some(pending) = app.image_pending.remove(&avatar_id) {
                    let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
                    let tex = ui.ctx().load_texture(&avatar_id, color, egui::TextureOptions::default());
                    app.image_textures.insert(avatar_id.clone(), tex.clone());
                    ui.add(egui::Image::from_texture(&tex).max_width(24.0 * zoom).rounding(12.0 * zoom));
                } else if !app.image_loading.contains(&avatar_id) && !app.image_errors.contains_key(&avatar_id) {
                    let url = crate::app::resolve_blob_url(&app.base_url_input, &avatar_id);
                    app.spawn_load_image(&avatar_id, &url);
                }
            }

            let name = peer.as_ref().and_then(|p| p.username.as_deref()).unwrap_or(author_id);
            if ui.add(egui::Link::new(RichText::new(name).strong().size(12.0 * zoom))).clicked() {
                app.show_identity = true;
                app.identity_state.inspected_peer = peer;
            }
        }

        if ui.button(RichText::new(format!("#{}", post.id)).monospace().size(11.0 * zoom)).clicked() {
            state.selected_post = Some(post.id.clone());
        }

        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(RichText::new(format_timestamp(&post.created_at)).color(Color32::from_rgb(200, 200, 210)).size(10.0 * zoom));
        });
    });
}

fn render_post_link(
    ui: &mut egui::Ui,
    state: &mut ThreadState,
    target_id: &str,
    zoom: f32,
    viewport: egui::Rect,
    is_parent_cursor: bool,
    is_reply_cursor: bool,
    is_reply_link: bool,
) {
    let short_id = if target_id.len() > 8 { &target_id[..8] } else { target_id };
    let text = RichText::new(format!(">>{}", short_id)).size(11.0 * zoom);

    // Blue background for parent cursor, red for reply cursor
    let text = if is_parent_cursor {
        text.background_color(Color32::from_rgb(60, 100, 200)) // Blue
    } else if is_reply_cursor {
        text.background_color(Color32::from_rgb(200, 60, 60)) // Red
    } else {
        text
    };

    let link = ui.link(text);
    
    if link.clicked() {
        state.selected_post = Some(target_id.to_string());
        if let Some(node) = state.graph_nodes.get(target_id) {
            let center = viewport.center();
            let target_x = node.pos.x * zoom;
            let target_y = node.pos.y * zoom;
            let half_width = (400.0 * zoom) / 2.0; 
            let half_height = (100.0 * zoom) / 2.0; 
            state.graph_offset.x = (center.x - viewport.left()) - (target_x + half_width);
            state.graph_offset.y = (center.y - viewport.top()) - (target_y + half_height);
        }
    }
    
    link.on_hover_ui(|ui| {
        if let Some(details) = &state.details {
            if let Some(post) = details.posts.iter().find(|p| p.id == target_id) {
                ui.set_max_width(300.0);
                ui.horizontal(|ui| {
                    let author = post.author_peer_id.as_deref().unwrap_or("Anon");
                    ui.label(RichText::new(author).strong());
                    ui.label(RichText::new(format_timestamp(&post.created_at)).color(Color32::GRAY));
                });
                ui.separator();
                let preview_len = post.body.len().min(200);
                let preview = &post.body[..preview_len];
                ui.label(format!("{}{}", preview, if post.body.len() > 200 { "..." } else { "" }));
            } else {
                ui.label("Post not found");
            }
        }
    });
}

fn render_node_actions(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView, zoom: f32) {
    ui.horizontal(|ui| {
        if ui.button(RichText::new("↩ Reply").size(13.0 * zoom)).clicked() {
            GraphchanApp::set_reply_target(state, &post.id);
        }
        if ui.button(RichText::new("❝ Quote").size(13.0 * zoom)).clicked() {
            GraphchanApp::quote_post(state, &post.id);
        }
    });

    // Reactions
    ui.add_space(4.0 * zoom);
    render_node_reactions(app, ui, state, &post.id, zoom);
}

fn render_node_reactions(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState, post_id: &str, zoom: f32) {
    // Load reactions if not already loaded/loading
    if !state.reactions.contains_key(post_id) && !state.reactions_loading.contains(post_id) {
        state.reactions_loading.insert(post_id.to_string());
        app.spawn_load_reactions(post_id);
    }

    ui.horizontal_wrapped(|ui| {
        // Show existing reactions
        if let Some(reactions_response) = state.reactions.get(post_id) {
            for (emoji, count) in &reactions_response.counts {
                let button_text = format!("{} {}", emoji, count);
                let button = ui.add(egui::Button::new(RichText::new(button_text).size(12.0 * zoom)));
                if button.clicked() {
                    // Toggle reaction: remove if user already reacted, otherwise add
                    let self_peer_id = app.identity_state.local_peer.as_ref().map(|p| p.id.clone());
                    if let Some(self_id) = self_peer_id {
                        let has_reacted = reactions_response.reactions.iter()
                            .any(|r| r.emoji == *emoji && r.reactor_peer_id == self_id);

                        if has_reacted {
                            app.spawn_remove_reaction(post_id, emoji);
                        } else {
                            app.spawn_add_reaction(post_id, emoji);
                        }
                    }
                }
            }
        }

        // "+" button to open emoji picker
        let picker_open = state.emoji_picker_open.as_ref() == Some(&post_id.to_string());
        let button = ui.add(egui::Button::new(RichText::new(if picker_open { "−" } else { "+" }).size(12.0 * zoom)));
        if button.clicked() {
            if picker_open {
                state.emoji_picker_open = None;
            } else {
                state.emoji_picker_open = Some(post_id.to_string());
            }
        }

        // Show emoji picker if open for this post
        if picker_open {
            ui.separator();
            ui.label(RichText::new("Quick:").size(11.0 * zoom));
            let quick_emojis = ["👍", "❤️", "😂", "🎉", "🤔", "👀", "🔥", "✨"];
            for emoji in quick_emojis {
                if ui.add(egui::Button::new(RichText::new(emoji).size(12.0 * zoom))).clicked() {
                    app.spawn_add_reaction(post_id, emoji);
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

fn render_node_attachments(app: &mut GraphchanApp, ui: &mut egui::Ui, attachments: Option<&Vec<FileResponse>>, api_base: &str, zoom: f32) {
    // Handle Audio Promise
    let mut new_audio_data = None;
    if let ViewState::Thread(state) = &mut app.view {
         if state.audio_promise.as_ref().map(|p| p.ready().is_some()).unwrap_or(false) {
             new_audio_data = state.audio_promise.take().map(|p| p.block_and_take());
         }
    }
    
    if let Some(Ok((id, data))) = new_audio_data {
        if let ViewState::Thread(state) = &mut app.view {
             // Play logic
             if state.audio_stream.is_none() {
                 if let Ok((stream, handle)) = rodio::OutputStream::try_default() {
                     state.audio_stream = Some(stream);
                     state.audio_handle = Some(handle);
                 }
             }
             if let Some(handle) = &state.audio_handle {
                if let Ok(sink) = rodio::Sink::try_new(handle) {
                    let cursor = std::io::Cursor::new(data);
                    if let Ok(decoder) = rodio::Decoder::new(cursor) {
                        sink.append(decoder);
                        sink.play();
                        state.audio_sink = Some(sink);
                        state.currently_playing = Some(id);
                    }
                }
            }
        }
    }

    let files = match attachments {
        Some(list) => list,
        None => return,
    };

    if files.is_empty() {
        return;
    }

    ui.add_space(6.0 * zoom);

    for file in files {
        let mime = file.mime.as_deref().unwrap_or("");
        if mime.starts_with("image/") {
            match image_preview(app, ui, file, api_base) {
                ImagePreview::Ready(tex) => {
                    let resp = ui.add(
                        egui::Image::from_texture(&tex)
                            .maintain_aspect_ratio(true)
                            .max_width(120.0 * zoom)
                            .max_height(120.0 * zoom)
                            .sense(egui::Sense::click()),
                    );
                    if resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    if resp.clicked() {
                        app.image_viewers.insert(file.id.clone(), true);
                    }
                }
                ImagePreview::Loading => {
                    ui.horizontal(|ui| {
                        ui.add(egui::Spinner::new().size(16.0 * zoom));
                        ui.label(RichText::new("Loading image…").size(12.0 * zoom));
                    });
                }
                ImagePreview::Error(err) => {
                    ui.colored_label(Color32::LIGHT_RED, RichText::new(format!("Image error: {err}")).size(11.0 * zoom));
                    let download_url = crate::app::resolve_download_url(api_base, file.download_url.as_deref(), &file.id);
                    let name = file.original_name.as_deref().unwrap_or("attachment");
                    ui.hyperlink_to(RichText::new(name).size(11.0 * zoom), download_url);
                }
                ImagePreview::None => {}
            }
        } else if mime.starts_with("audio/") {
             // Audio Player UI
             ui.horizontal(|ui| {
                 let is_playing = if let ViewState::Thread(state) = &app.view {
                     state.currently_playing.as_deref() == Some(&file.id)
                 } else { false };
                 
                 let icon = if is_playing { "⏸" } else { "▶" };
                 if ui.button(RichText::new(icon).size(14.0 * zoom)).clicked() {
                     if let ViewState::Thread(state) = &mut app.view {
                         if is_playing {
                             state.audio_sink = None;
                             state.currently_playing = None;
                         } else {
                             // Load and Play
                             let url = format!("{}/files/{}", api_base, file.id);
                             let file_id = file.id.clone();
                             state.audio_promise = Some(poll_promise::Promise::spawn_thread("load_audio", move || {
                                 let bytes = reqwest::blocking::get(url)
                                     .map_err(|e| e.to_string())?
                                     .bytes()
                                     .map_err(|e| e.to_string())?
                                     .to_vec();
                                 Ok((file_id, bytes))
                             }));
                         }
                     }
                 }
                 ui.label(RichText::new(file.original_name.as_deref().unwrap_or("audio")).size(12.0 * zoom));
             });
        } else {
            let name = file.original_name.as_deref().unwrap_or("attachment");
            let mime = file.mime.as_deref().unwrap_or("");
            let icon = if mime.starts_with("video/") { "🎥" } else if mime.starts_with("text/") || mime.contains("markdown") { "📄" } else if mime.contains("pdf") { "📕" } else { "📎" };
            let label = if file.present { format!("{} {}", icon, name) } else { format!("{} {} (remote)", icon, name) };
            let response = ui.add(egui::Label::new(RichText::new(label).size(11.0 * zoom).underline().color(Color32::LIGHT_BLUE)).sense(egui::Sense::click()));
            if response.clicked() {
                app.open_file_viewer(&file.id, name, mime, api_base);
            }
        }
        ui.add_space(4.0 * zoom);
    }
}

pub fn is_image(file: &FileResponse) -> bool {
    file.present && file.mime.as_deref().map(|m| m.starts_with("image/")).unwrap_or(false)
}

pub fn estimate_node_height(ui: &egui::Ui, post: &PostView, has_preview: bool, children_count: usize, zoom: f32, card_width: f32) -> f32 {
    use eframe::egui::FontId;
    let text_width = (card_width - 20.0) * zoom;
    let text_height = ui.fonts(|fonts| {
        let body = post.body.clone();
        let galley = fonts.layout(body, FontId::proportional(13.0 * zoom), Color32::WHITE, text_width);
        galley.size().y
    });
    let mut height = (50.0 * zoom) + text_height;
    height += 25.0 * zoom; // Header
    if !post.parent_post_ids.is_empty() { height += 20.0 * zoom; }

    // More accurate attachment height calculation
    if has_preview {
        // Count actual image files
        let image_count = post.files.iter()
            .filter(|f| f.present && f.mime.as_deref().map(|m| m.starts_with("image/")).unwrap_or(false))
            .count();

        if image_count > 0 {
            height += 6.0 * zoom; // Initial spacing before attachments
            // Each image: max_height (120) + spacing (4)
            height += (image_count as f32) * (120.0 + 4.0) * zoom;
        }
    }

    // Account for non-image attachments (audio, video, files)
    let other_attachments = post.files.iter()
        .filter(|f| !f.mime.as_deref().map(|m| m.starts_with("image/")).unwrap_or(false))
        .count();
    if other_attachments > 0 {
        height += 6.0 * zoom; // Spacing before attachments if not already added
        height += (other_attachments as f32) * (25.0 * zoom); // Approx height per non-image attachment
    }

    if children_count > 0 {
        // Estimate height of replies section
        // Approx 80px per child link (10 chars + spacing)
        let link_width = 80.0 * zoom;
        let available_width = text_width;
        let links_per_line = (available_width / link_width).max(1.0).floor();
        let lines = (children_count as f32 / links_per_line).ceil();
        let line_height = 15.0 * zoom;
        height += lines * line_height + (10.0 * zoom); // + padding
    }

    height + (40.0 * zoom) // Footer/Padding (increased from 20 to 40 for reactions and actions)
}

pub fn estimate_node_size(ui: &egui::Ui, post: &PostView, has_preview: bool, children_count: usize) -> egui::Vec2 {
    use eframe::egui::{FontId, TextStyle};
    
    // Calculate Width (Unscaled)
    let body_size = ui.style().text_styles.get(&TextStyle::Body).map(|f| f.size).unwrap_or(14.0);
    let font_id = FontId::proportional(body_size);
    
    let longest_line_width = ui.fonts(|fonts| {
        post.body.lines()
            .map(|line| fonts.layout_no_wrap(line.to_string(), font_id.clone(), Color32::WHITE).size().x)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(0.0)
    });
    
    let content_width = longest_line_width + 30.0; // + Padding
    let min_width = 300.0;
    let max_width = 1200.0;
    let width = content_width.clamp(min_width, max_width);
    
    let height = estimate_node_height(ui, post, has_preview, children_count, 1.0, width);
    
    egui::vec2(width, height)
}
