use eframe::egui::{self, Color32, Margin, RichText};
use crate::models::{FileResponse, PostView};
use super::super::state::ThreadState;
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
) -> egui::Response {
    let rect_node = layout.rect;
    let selected = state.selected_post.as_ref() == Some(&layout.post.id);
    let reply_target = state.reply_to.iter().any(|id| id == &layout.post.id);
    
    let naturally_hovered = ui.ctx().pointer_hover_pos()
        .map(|pos| rect_node.contains(pos))
        .unwrap_or(false);
    let locked_hover = state.locked_hover_post.as_ref() == Some(&layout.post.id);
    let hovered = naturally_hovered || locked_hover;

    let fill_color = if reply_target {
        Color32::from_rgb(40, 52, 85)
    } else if selected {
        Color32::from_rgb(58, 48, 24)
    } else if hovered {
        Color32::from_rgb(42, 45, 60)
    } else {
        Color32::from_rgb(30, 30, 38)
    };

    let stroke_width = (if hovered { 2.5 } else { 1.5 }) * zoom;
    let stroke_color = if reply_target {
        Color32::from_rgb(255, 190, 92)
    } else if selected {
        Color32::from_rgb(250, 208, 108)
    } else if hovered {
        Color32::from_rgb(120, 140, 200)
    } else {
        Color32::from_rgb(80, 90, 130)
    };

    let rounding = egui::Rounding::same(10.0 * zoom);
    ui.painter().rect(
        rect_node,
        rounding,
        fill_color,
        egui::Stroke::new(stroke_width, stroke_color),
    );

    let response = ui.allocate_new_ui(egui::UiBuilder::new().max_rect(rect_node), |ui| {
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

        egui::Frame::none()
            .inner_margin(Margin::same(10.0 * zoom))
            .show(ui, |ui| {
                ui.set_clip_rect(rect_node.intersect(viewport));
                
                ui.vertical(|ui| {
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        if !children.is_empty() {
                            ui.add_space(4.0 * zoom);
                            ui.horizontal_wrapped(|ui| {
                                ui.label(RichText::new("↪ Replies:").size(11.0 * zoom).color(Color32::GRAY));
                                for child_id in children {
                                    render_post_link(ui, state, child_id, zoom, viewport);
                                }
                            });
                            ui.add_space(4.0 * zoom);
                        }

                        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                            render_node_header(app, ui, state, &layout.post, zoom);

                            if !layout.post.parent_post_ids.is_empty() {
                                ui.add_space(4.0 * zoom);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("↩ Replying to:").size(11.0 * zoom).color(Color32::GRAY));
                                    for parent_id in &layout.post.parent_post_ids {
                                        render_post_link(ui, state, parent_id, zoom, viewport);
                                    }
                                });
                            }

                            ui.add_space(6.0 * zoom);
                            
                            render_post_body(ui, &layout.post.body);
                            
                            render_node_attachments(app, ui, layout.attachments.as_ref(), api_base, zoom);

                            ui.add_space(6.0 * zoom);
                            render_node_actions(ui, state, &layout.post, zoom);
                        });
                    });
                });
            })
    }).response;

    if response.clicked() {
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

fn render_post_link(ui: &mut egui::Ui, state: &mut ThreadState, target_id: &str, zoom: f32, viewport: egui::Rect) {
    let short_id = if target_id.len() > 8 { &target_id[..8] } else { target_id };
    let link = ui.link(RichText::new(format!(">>{}", short_id)).size(11.0 * zoom));
    
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

fn render_node_actions(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView, zoom: f32) {
    ui.horizontal(|ui| {
        if ui.button(RichText::new("↩ Reply").size(13.0 * zoom)).clicked() {
            GraphchanApp::set_reply_target(state, &post.id);
        }
        if ui.button(RichText::new("❝ Quote").size(13.0 * zoom)).clicked() {
            GraphchanApp::quote_post(state, &post.id);
        }
    });
}

fn render_node_attachments(app: &mut GraphchanApp, ui: &mut egui::Ui, attachments: Option<&Vec<FileResponse>>, api_base: &str, zoom: f32) {
    let files = match attachments {
        Some(list) => list,
        None => return,
    };

    if files.is_empty() {
        return;
    }

    ui.add_space(6.0 * zoom);

    for file in files {
        if is_image(file) {
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

pub fn estimate_node_height(ui: &egui::Ui, post: &PostView, has_preview: bool, has_children: bool, zoom: f32, card_width: f32) -> f32 {
    use eframe::egui::FontId;
    
    let text_width = (card_width - 20.0) * zoom;
    let font_id = FontId::proportional(13.0 * zoom);
    
    // Simulate render_post_body wrapping
    let mut text_height = 0.0;
    let space_width = ui.fonts(|f| f.layout_no_wrap(" ".to_string(), font_id.clone(), Color32::WHITE).size().x);
    let line_spacing = ui.spacing().item_spacing.y * zoom; // scaled spacing
    let row_height = ui.fonts(|f| f.row_height(&font_id));
    
    for line in post.body.lines() {
        let mut current_x = 0.0;
        let mut rows = 1;
        
        for (i, word) in line.split(' ').enumerate() {
            let word_width = ui.fonts(|f| f.layout_no_wrap(word.to_string(), font_id.clone(), Color32::WHITE).size().x);
            
            if i > 0 {
                // Add space
                if current_x + space_width > text_width {
                    current_x = 0.0;
                    rows += 1;
                }
                current_x += space_width;
            }
            
            if current_x + word_width > text_width {
                if current_x > 0.0 {
                    current_x = 0.0;
                    rows += 1;
                }
            }
            current_x += word_width;
        }
        
        text_height += rows as f32 * row_height;
        // Add spacing between paragraphs if multiple lines? 
        // render_post_body creates a new horizontal_wrapped for each line.
        // horizontal_wrapped adds item_spacing.y between them?
        // No, horizontal_wrapped is a Ui block.
        // ui.vertical puts them one after another.
        // So yes, item_spacing.y is added between paragraphs.
    }
    
    // Add spacing between paragraphs
    let num_paragraphs = post.body.lines().count();
    if num_paragraphs > 1 {
        text_height += (num_paragraphs - 1) as f32 * line_spacing;
    }

    let mut height = (50.0 * zoom) + text_height;
    height += 25.0 * zoom; // Header
    if !post.parent_post_ids.is_empty() { height += 20.0 * zoom; }
    if has_preview { height += 126.0 * zoom; }
    if has_children { height += 20.0 * zoom; }
    height + (20.0 * zoom) // Footer/Padding
}
