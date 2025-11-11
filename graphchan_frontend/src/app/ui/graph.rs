use std::collections::HashMap;
use std::sync::OnceLock;
use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Margin, Pos2, RichText};
use log::debug;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::models::{FileResponse, PostView};

use super::super::state::{GraphNode, ThreadState};
use super::super::{format_timestamp, GraphchanApp};

static START_TIME: OnceLock<Instant> = OnceLock::new();

struct NodeLayoutData {
    post: PostView,
    rect: egui::Rect,
    attachments: Option<Vec<FileResponse>>,
}

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

    for post in &posts {
        debug!(
            "graph node post={} parents={:?} body_len={}",
            post.id,
            post.parent_post_ids,
            post.body.len()
        );
    }

    let sim_active = START_TIME.get_or_init(Instant::now).elapsed().as_secs_f32() < 1.0;

    if sim_active && !state.graph_dragging {
        for _ in 0..2 {
            step_graph_layout(&mut state.graph_nodes, &posts);
        }
    }

    let available = ui.available_size();
    let (rect, response) = ui.allocate_at_least(available, egui::Sense::click_and_drag());
    let canvas = ui.painter().with_clip_rect(rect);
    canvas.rect_filled(rect, 0.0, Color32::from_rgb(12, 13, 20));
    let dot_spacing = 28.0;
    let dot_color = Color32::from_rgba_premultiplied(70, 72, 95, 90);
    let mut x = rect.left();
    while x < rect.right() {
        let mut y = rect.top();
        while y < rect.bottom() {
            canvas.circle_filled(egui::pos2(x, y), 1.0, dot_color);
            y += dot_spacing;
        }
        x += dot_spacing;
    }

    let edge_painter = ui.painter().with_clip_rect(rect);

    if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pointer_pos) {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let old_zoom = state.graph_zoom;
                let zoom_factor = (1.0 + scroll * 0.001).clamp(0.5, 3.5);
                let target_zoom = (old_zoom * zoom_factor).clamp(0.2, 4.0);

                let rect_size = rect.size();
                let anchor = pointer_pos;
                let world_x = ((anchor.x - rect.left() - state.graph_offset.x)
                    / (rect_size.x * old_zoom))
                    .clamp(0.0, 1.0);
                let world_y = ((anchor.y - rect.top() - state.graph_offset.y)
                    / (rect_size.y * old_zoom))
                    .clamp(0.0, 1.0);

                state.graph_zoom = target_zoom;
                state.graph_offset.x =
                    anchor.x - rect.left() - world_x * rect_size.x * state.graph_zoom;
                state.graph_offset.y =
                    anchor.y - rect.top() - world_y * rect_size.y * state.graph_zoom;
            }
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

    let mut layouts = Vec::new();
    for post in posts.iter() {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            let attachments = state.attachments.get(&post.id).cloned();
            let card_width = 320.0;
            let has_preview = attachments
                .as_ref()
                .map(|files| files.iter().any(is_image))
                .unwrap_or(false);
            let card_height = estimate_node_height(ui, post, has_preview);
            node.size = egui::vec2(card_width, card_height);
            let top_left = world_to_screen(node.pos);
            let size = node.size * state.graph_zoom;
            let rect_node = egui::Rect::from_min_size(top_left, size);
            layouts.push(NodeLayoutData {
                post: post.clone(),
                rect: rect_node,
                attachments,
            });
        }
    }

    let rect_lookup: HashMap<String, egui::Rect> = layouts
        .iter()
        .map(|layout| (layout.post.id.clone(), layout.rect))
        .collect();

    draw_edges(&edge_painter, &layouts, &rect_lookup, state);

    let api_base = app.api.base_url().to_string();

    for layout in layouts {
        let rect_node = layout.rect;
        let drag_id = ui.make_persistent_id(format!("graph_node_drag_{}", layout.post.id));
        let drag_handle = ui.interact(rect_node, drag_id, egui::Sense::click_and_drag());
        let hovered = rect_node.contains(response.hover_pos().unwrap_or_default());

        if state.graph_dragging {
            if let Some(pos) = response.hover_pos() {
                if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                    if node.dragging {
                        let wx = ((pos.x - rect.left() - state.graph_offset.x)
                            / (rect.width() * state.graph_zoom))
                            .clamp(0.0, 1.0);
                        let wy = ((pos.y - rect.top() - state.graph_offset.y)
                            / (rect.height() * state.graph_zoom))
                            .clamp(0.0, 1.0);
                        node.pos = egui::pos2(wx, wy);
                    }
                }
            }
        }

        let selected = state.selected_post.as_ref() == Some(&layout.post.id);
        let reply_target = state.reply_to.iter().any(|id| id == &layout.post.id);
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

        let card_response = ui
            .allocate_ui_at_rect(rect_node, |ui| {
                ui.set_clip_rect(rect_node);
                egui::Frame::none()
                    .fill(fill_color)
                    .stroke(egui::Stroke::new(1.5, stroke_color))
                    .rounding(egui::Rounding::same(10.0))
                    .inner_margin(Margin::same(10.0))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            render_node_header(ui, state, &layout.post);

                            if !layout.post.parent_post_ids.is_empty() {
                                ui.add_space(4.0);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Replying to:").size(11.0));
                                    for parent in &layout.post.parent_post_ids {
                                        if ui.link(format!("#{parent}")).clicked() {
                                            state.selected_post = Some(parent.clone());
                                        }
                                    }
                                });
                            }

                            ui.add_space(6.0);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&layout.post.body)
                                        .size(13.0)
                                        .color(Color32::from_rgb(220, 220, 230)),
                                )
                                .wrap(true),
                            );

                            render_node_attachments(
                                app,
                                ui,
                                layout.attachments.as_ref(),
                                &api_base,
                            );

                            ui.add_space(6.0);
                            render_node_actions(ui, state, &layout.post);
                        });
                    });
            })
            .response;

        let combined_drag = drag_handle.union(card_response);

        if combined_drag.drag_started() {
            if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                node.dragging = true;
            }
            state.graph_dragging = true;
        }
        if combined_drag.drag_stopped() {
            if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                node.dragging = false;
            }
            state.graph_dragging = false;
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
    attachments: Option<&Vec<FileResponse>>,
    api_base: &str,
) {
    let files = match attachments {
        Some(list) => list,
        None => return,
    };

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

pub(super) fn image_preview(
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
        let url =
            super::super::resolve_download_url(api_base, file.download_url.as_deref(), &file.id);
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

pub(super) enum ImagePreview {
    Ready(egui::TextureHandle),
    Loading,
    Error(String),
    None,
}

fn draw_edges(
    painter: &egui::Painter,
    layouts: &[NodeLayoutData],
    rect_lookup: &HashMap<String, egui::Rect>,
    state: &ThreadState,
) {
    let reply_targets = &state.reply_to;
    for layout in layouts {
        for parent in &layout.post.parent_post_ids {
            let (parent_rect, child_rect) =
                match (rect_lookup.get(parent), rect_lookup.get(&layout.post.id)) {
                    (Some(p), Some(c)) => (p, c),
                    _ => continue,
                };

            let start = anchor_bottom(parent_rect);
            let end = anchor_top(child_rect);
            let is_reply_edge = reply_targets
                .iter()
                .any(|id| id == parent || id == &layout.post.id);
            let sel = state.selected_post.as_ref();
            let color = if is_reply_edge {
                Color32::from_rgb(255, 190, 92)
            } else if sel == Some(&layout.post.id) || sel == Some(parent) {
                Color32::from_rgb(110, 190, 255)
            } else {
                Color32::from_rgb(90, 110, 170)
            };

            painter.line_segment(
                [start, end],
                egui::Stroke::new(if is_reply_edge { 3.4 } else { 2.0 }, color),
            );

            draw_arrow(painter, start, end, color);
        }
    }
}

fn anchor_top(rect: &egui::Rect) -> Pos2 {
    pos_with_offset(rect.center_top(), 0.0, -6.0)
}

fn anchor_bottom(rect: &egui::Rect) -> Pos2 {
    pos_with_offset(rect.center_bottom(), 0.0, 6.0)
}

fn pos_with_offset(mut pos: Pos2, dx: f32, dy: f32) -> Pos2 {
    pos.x += dx;
    pos.y += dy;
    pos
}

fn draw_arrow(painter: &egui::Painter, start: Pos2, end: Pos2, color: Color32) {
    let dir = (end - start).normalized();
    let normal = egui::Vec2::new(-dir.y, dir.x);
    let arrow_size = 8.0;
    let arrow_tip = end;
    let arrow_left = egui::pos2(
        end.x - dir.x * arrow_size + normal.x * arrow_size * 0.5,
        end.y - dir.y * arrow_size + normal.y * arrow_size * 0.5,
    );
    let arrow_right = egui::pos2(
        end.x - dir.x * arrow_size - normal.x * arrow_size * 0.5,
        end.y - dir.y * arrow_size - normal.y * arrow_size * 0.5,
    );
    painter.add(egui::Shape::convex_polygon(
        vec![arrow_tip, arrow_left, arrow_right],
        color,
        egui::Stroke::NONE,
    ));
}

fn estimate_node_height(ui: &egui::Ui, post: &PostView, has_preview: bool) -> f32 {
    let text_width = 320.0 - 20.0;
    let text_height = ui.fonts(|fonts| {
        let body = post.body.clone();
        let galley = fonts.layout(body, FontId::proportional(13.0), Color32::WHITE, text_width);
        galley.size().y
    });

    let mut height = 70.0 + text_height; // header + body
    if !post.parent_post_ids.is_empty() {
        height += 28.0;
    }
    if has_preview {
        height += 120.0;
    }
    height + 36.0 // actions + padding
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
