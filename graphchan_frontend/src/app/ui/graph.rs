use std::collections::HashMap;
use std::time::Instant;

use eframe::egui::{self, Color32, RichText};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::models::{FileResponse, PostView};

use super::super::state::{GraphNode, ThreadState};
use super::super::{format_timestamp, GraphchanApp};

static mut START_TIME: Option<Instant> = None;

pub(crate) fn build_initial_graph(posts: &[PostView]) -> HashMap<String, GraphNode> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut nodes = HashMap::new();
    for p in posts {
        let x = rng.gen::<f32>() * 0.8 + 0.1;
        let y = rng.gen::<f32>() * 0.8 + 0.1;
        nodes.insert(
            p.id.clone(),
            GraphNode {
                pos: egui::pos2(x, y),
                vel: egui::vec2(0.0, 0.0),
                id: p.id.clone(),
                size: egui::vec2(220.0, 140.0),
                dragging: false,
            },
        );
    }
    nodes
}

pub(crate) fn render_graph(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState) {
    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };

    let sim_active = unsafe {
        if START_TIME.is_none() {
            START_TIME = Some(Instant::now());
        }
        START_TIME
            .as_ref()
            .map(|t| t.elapsed().as_secs_f32() < 1.0)
            .unwrap_or(false)
    };

    if sim_active && !state.graph_dragging {
        for _ in 0..2 {
            step_graph_layout(&mut state.graph_nodes, &posts);
        }
    }

    let available = ui.available_size();
    let (rect, response) = ui.allocate_at_least(available, egui::Sense::click_and_drag());
    let painter = ui.painter_at(rect);

    if response.hovered() {
        let scroll = ui.input(|i| i.raw_scroll_delta.y);
        if scroll != 0.0 {
            state.graph_zoom *= 1.0 + scroll * 0.001;
            state.graph_zoom = state.graph_zoom.clamp(0.2, 3.0);
        }
    }
    if response.dragged_by(egui::PointerButton::Secondary) {
        state.graph_offset += response.drag_delta();
    }

    let rect_left = rect.left();
    let rect_top = rect.top();
    let rect_width = rect.width();
    let rect_height = rect.height();
    let offset = state.graph_offset;
    let zoom = state.graph_zoom;
    let world_to_screen = |p: egui::Pos2| {
        egui::pos2(
            rect_left + offset.x + p.x * rect_width * zoom,
            rect_top + offset.y + p.y * rect_height * zoom,
        )
    };

    let reply_targets = state.reply_to.clone();

    for post in posts.iter() {
        for parent in &post.parent_post_ids {
            if let (Some(a), Some(b)) = (
                state.graph_nodes.get(parent),
                state.graph_nodes.get(&post.id),
            ) {
                let pa = world_to_screen(a.pos);
                let pb = world_to_screen(b.pos);
                let is_reply_edge = reply_targets
                    .iter()
                    .any(|id| id == parent || id == &post.id);
                let sel = state.selected_post.as_ref();
                let color = if is_reply_edge {
                    Color32::from_rgb(255, 190, 92)
                } else if sel == Some(&post.id) || sel == Some(parent) {
                    Color32::from_rgb(120, 200, 255)
                } else {
                    Color32::from_rgb(90, 90, 110)
                };
                painter.line_segment(
                    [pa, pb],
                    egui::Stroke {
                        width: if is_reply_edge { 3.0 } else { 1.5 },
                        color,
                    },
                );
            } else if state.graph_nodes.get(parent).is_none() {
                let placeholder = GraphNode {
                    pos: egui::pos2(0.05, 0.05),
                    vel: egui::vec2(0.0, 0.0),
                    id: parent.clone(),
                    size: egui::vec2(220.0, 140.0),
                    dragging: false,
                };
                state.graph_nodes.insert(parent.clone(), placeholder);
            }
        }
    }

    let api_base = app.api.base_url().to_string();

    for post in posts.iter() {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            let has_preview = state
                .attachments
                .get(&post.id)
                .map(|files| files.iter().any(is_image))
                .unwrap_or(false);
            let card_width = 280.0;
            let card_height = if has_preview { 230.0 } else { 180.0 };
            node.size = egui::vec2(card_width, card_height);

            let top_left = world_to_screen(node.pos);
            let size = node.size * state.graph_zoom;
            let rect_node = egui::Rect::from_min_size(top_left, size);
            let drag_id = ui.make_persistent_id(format!("graph_node_drag_{}", post.id));
            let drag_response = ui.interact(rect_node, drag_id, egui::Sense::click_and_drag());
            let hovered = drag_response.hovered();

            if drag_response.dragged_by(egui::PointerButton::Primary) {
                if let Some(current) = state.graph_nodes.get_mut(&post.id) {
                    current.dragging = true;
                    state.graph_dragging = true;
                }
            }
            if drag_response.drag_stopped() {
                if let Some(current) = state.graph_nodes.get_mut(&post.id) {
                    current.dragging = false;
                    state.graph_dragging = false;
                }
            }

            if state.graph_dragging {
                if let Some(pos) = response.hover_pos() {
                    if let Some(current) = state.graph_nodes.get_mut(&post.id) {
                        if current.dragging {
                            let wx = ((pos.x - rect.left() - state.graph_offset.x)
                                / (rect.width() * state.graph_zoom))
                                .clamp(0.0, 1.0);
                            let wy = ((pos.y - rect.top() - state.graph_offset.y)
                                / (rect.height() * state.graph_zoom))
                                .clamp(0.0, 1.0);
                            current.pos = egui::pos2(wx, wy);
                        }
                    }
                }
            }

            let selected = state.selected_post.as_ref() == Some(&post.id);
            let reply_target = state.reply_to.iter().any(|id| id == &post.id);
            let fill_color = if reply_target {
                Color32::from_rgb(40, 52, 85)
            } else if selected {
                Color32::from_rgb(58, 48, 24)
            } else if hovered {
                Color32::from_rgb(38, 41, 54)
            } else {
                Color32::from_rgb(30, 30, 38)
            };

            let stroke_color = if reply_target {
                Color32::from_rgb(255, 190, 92)
            } else if selected {
                Color32::from_rgb(250, 208, 108)
            } else {
                Color32::from_rgb(80, 90, 130)
            };

            ui.allocate_ui_at_rect(rect_node, |ui| {
                ui.set_clip_rect(rect_node);
                egui::Frame::none()
                    .fill(fill_color)
                    .stroke(egui::Stroke::new(1.5, stroke_color))
                    .rounding(egui::Rounding::same(10.0))
                    .inner_margin(egui::Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            render_node_header(ui, state, post);

                            if !post.parent_post_ids.is_empty() {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Replying to:").size(11.0));
                                    for parent in &post.parent_post_ids {
                                        if ui.link(format!("#{parent}")).clicked() {
                                            state.selected_post = Some(parent.clone());
                                        }
                                    }
                                });
                            }

                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&post.body)
                                        .size(13.0)
                                        .color(Color32::from_rgb(220, 220, 230)),
                                )
                                .wrap(true),
                            );

                            if let Some(files) = state.attachments.get(&post.id) {
                                render_node_attachments(app, ui, files, &api_base);
                            }

                            ui.add_space(6.0);
                            render_node_actions(ui, state, post);
                        });
                    });
            });
        }
    }

    ui.separator();
    ui.label(format!(
        "Zoom: {:.2} | Nodes: {}",
        state.graph_zoom,
        state.graph_nodes.len()
    ));
}

fn render_node_header(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView) {
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new(format!("#{}", post.id)).monospace())
            .clicked()
        {
            state.selected_post = Some(post.id.clone());
        }
        let author = post.author_peer_id.as_deref().unwrap_or("Anonymous");
        ui.label(RichText::new(author).strong());
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format_timestamp(&post.created_at))
                    .color(Color32::from_rgb(200, 200, 210))
                    .size(11.0),
            );
        });
    });
}

fn render_node_actions(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView) {
    ui.horizontal(|ui| {
        if ui.button("↩ Reply").clicked() {
            GraphchanApp::set_reply_target(state, &post.id);
        }
        if ui.button("❝ Quote").clicked() {
            GraphchanApp::quote_post(state, &post.id);
        }
    });
}

fn render_node_attachments(
    app: &mut GraphchanApp,
    ui: &mut egui::Ui,
    files: &[FileResponse],
    api_base: &str,
) {
    if let Some(file) = files.iter().find(|f| is_image(f)) {
        match image_preview(app, ui, file, api_base) {
            ImagePreview::Ready(tex) => {
                ui.add_space(6.0);
                ui.add(
                    egui::Image::from_texture(&tex)
                        .maintain_aspect_ratio(true)
                        .max_width(120.0),
                );
            }
            ImagePreview::Loading => {
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label("Fetching image…");
                });
            }
            ImagePreview::Error(err) => {
                ui.add_space(6.0);
                ui.colored_label(Color32::LIGHT_RED, format!("Image failed: {err}"));
            }
            ImagePreview::None => {}
        }
    }
}

fn image_preview(
    app: &mut GraphchanApp,
    ui: &egui::Ui,
    file: &FileResponse,
    api_base: &str,
) -> ImagePreview {
    if !file.present {
        return ImagePreview::None;
    }

    if let Some(err) = app.image_errors.get(&file.id) {
        return ImagePreview::Error(err.clone());
    }

    if let Some(tex) = app.image_textures.get(&file.id) {
        return ImagePreview::Ready(tex.clone());
    }

    if let Some(pending) = app.image_pending.remove(&file.id) {
        let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
        let tex = ui
            .ctx()
            .load_texture(&file.id, color, egui::TextureOptions::default());
        app.image_textures.insert(file.id.clone(), tex.clone());
        return ImagePreview::Ready(tex);
    }

    if !app.image_loading.contains(&file.id) {
        let url = file
            .download_url
            .clone()
            .unwrap_or_else(|| format!("{api_base}/files/{}", file.id));
        app.spawn_download_image(&file.id, &url);
    }

    ImagePreview::Loading
}

fn is_image(file: &FileResponse) -> bool {
    file.present
        && file
            .mime
            .as_deref()
            .map(|m| m.starts_with("image/"))
            .unwrap_or(false)
}

enum ImagePreview {
    Ready(egui::TextureHandle),
    Loading,
    Error(String),
    None,
}

fn step_graph_layout(nodes: &mut HashMap<String, GraphNode>, posts: &[PostView]) {
    let repulsion = 0.0008;
    let attraction = 0.02;
    let damping = 0.9;
    let desired = 0.15;

    let mut edges = Vec::new();
    for post in posts {
        for parent in &post.parent_post_ids {
            if nodes.contains_key(parent) {
                edges.push((parent.clone(), post.id.clone()));
            }
        }
    }

    let ids: Vec<String> = nodes.keys().cloned().collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            let (ai, aj) = {
                let a = nodes.get(&ids[i]).unwrap();
                let b = nodes.get(&ids[j]).unwrap();
                (a.pos, b.pos)
            };
            let delta = aj - ai;
            let dist_sq = delta.length_sq().max(0.0001);
            let force = repulsion / dist_sq;
            let dir = if dist_sq > 0.0 {
                delta / dist_sq.sqrt()
            } else {
                egui::vec2(0.0, 0.0)
            };
            if let Some(node) = nodes.get_mut(&ids[i]) {
                node.vel -= dir * force;
            }
            if let Some(node) = nodes.get_mut(&ids[j]) {
                node.vel += dir * force;
            }
        }
    }

    for (a, b) in edges {
        let (pa, pb) = {
            let na = nodes.get(&a).unwrap();
            let nb = nodes.get(&b).unwrap();
            (na.pos, nb.pos)
        };
        let delta = pb - pa;
        let dist = delta.length().max(0.0001);
        let dir = delta / dist;
        let force = attraction * (dist - desired);
        if let Some(node) = nodes.get_mut(&a) {
            node.vel += dir * force;
        }
        if let Some(node) = nodes.get_mut(&b) {
            node.vel -= dir * force;
        }
    }

    for node in nodes.values_mut() {
        node.vel *= damping;
        node.pos += node.vel;
        node.pos.x = node.pos.x.clamp(0.0, 1.0);
        node.pos.y = node.pos.y.clamp(0.0, 1.0);
    }
}
