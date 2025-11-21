use std::collections::HashMap;

use std::time::Instant;

use eframe::egui::{self, Color32, FontId, Margin, Pos2, RichText};
use log::debug;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::models::{FileResponse, PostView};

use super::super::state::{GraphNode, ThreadState};
use super::super::{format_timestamp, GraphchanApp};



struct NodeLayoutData {
    post: PostView,
    rect: egui::Rect,
    attachments: Option<Vec<FileResponse>>,
}

pub(crate) fn build_initial_graph(posts: &[PostView]) -> HashMap<String, GraphNode> {
    let mut rng = StdRng::seed_from_u64(42);
    let mut nodes = HashMap::new();
    
    // Find OP (earliest post)
    let op_id = posts.iter().min_by_key(|p| &p.created_at).map(|p| p.id.clone());

    for p in posts {
        let (x, y) = if Some(&p.id) == op_id.as_ref() {
            (0.0, 0.0)
        } else {
            // Spawn others around center
            ((rng.gen::<f32>() - 0.5) * 10.0, (rng.gen::<f32>() - 0.5) * 10.0)
        };

        nodes.insert(
            p.id.clone(),
            GraphNode {
                pos: egui::pos2(x, y),
                vel: egui::vec2(0.0, 0.0),
                size: egui::vec2(320.0, 100.0), // Smaller default height
                dragging: false,
                pinned: Some(&p.id) == op_id.as_ref(),
            },
        );
    }
    nodes
}

// ... (existing code) ...

fn estimate_node_height(ui: &egui::Ui, post: &PostView, has_preview: bool) -> f32 {
    let text_width = 320.0 - 20.0;
    let text_height = ui.fonts(|fonts| {
        let body = post.body.clone();
        let galley = fonts.layout(body, FontId::proportional(13.0), Color32::WHITE, text_width);
        galley.size().y
    });

    let mut height = 40.0 + text_height; // header + body (reduced base)
    if !post.parent_post_ids.is_empty() {
        height += 20.0;
    }
    // Add space for replies
    height += 20.0; 
    
    if has_preview {
        height += 120.0; // Image height
    }
    
    height += 40.0; // Actions + padding. Reduced from 100.0 to match visual size better.
    height
}

fn step_graph_layout(nodes: &mut HashMap<String, GraphNode>, posts: &[PostView], scale: f32, thread_id: &str, repulsion_force: f32) {
    let repulsion = repulsion_force / 1000.0; // Scale down for physics
    let attraction = 0.015; // Increased from 0.002 to keep things tighter
    let damping = 0.80; // Increased damping for stability
    let desired = 1.5; // Reduced desired distance
    
    let mut edges = Vec::new();
    for post in posts {
        for parent in &post.parent_post_ids {
            if nodes.contains_key(parent) {
                edges.push((parent.clone(), post.id.clone()));
            }
        }
    }

    let ids: Vec<String> = nodes.keys().cloned().collect();
    
    // 1. Apply Forces
    for i in 0..ids.len() {
        // Skip OP forces
        if ids[i] == thread_id { continue; }
        
        for j in (i + 1)..ids.len() {
            let (ai, aj) = {
                let a = nodes.get(&ids[i]).unwrap();
                let b = nodes.get(&ids[j]).unwrap();
                (a.pos, b.pos)
            };
            let delta = aj - ai;
            let dist_sq = delta.length_sq().max(0.0001);
            
            // Soft repulsion
            let force = repulsion / dist_sq;
            let dir = if dist_sq > 0.0 {
                delta / dist_sq.sqrt()
            } else {
                egui::vec2(1.0, 0.0)
            };
            
            if let Some(node) = nodes.get_mut(&ids[i]) {
                if !node.pinned {
                    node.vel -= dir * force;
                }
            }
            if let Some(node) = nodes.get_mut(&ids[j]) {
                if !node.pinned {
                    node.vel += dir * force;
                }
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
        
        // Spring attraction
        let force = attraction * (dist - desired);
        
        if a != thread_id { // Keep thread_id check for edges? No, use pinned.
            if let Some(node) = nodes.get_mut(&a) {
                if !node.pinned {
                    node.vel += dir * force;
                }
            }
        }
        if b != thread_id {
            if let Some(node) = nodes.get_mut(&b) {
                if !node.pinned {
                    node.vel -= dir * force;
                }
            }
        }
    }

    // 2. Update Positions
    for (_id, node) in nodes.iter_mut() {
        if node.pinned {
            node.vel = egui::vec2(0.0, 0.0);
            continue;
        }
        
        node.vel *= damping;
        // Limit velocity to prevent explosions
        if node.vel.length() > 1.0 {
            node.vel = node.vel.normalized() * 1.0;
        }
        node.pos += node.vel;
    }

    // 3. Resolve Collisions (AABB) - REMOVED to allow overlap
    // We rely on soft repulsion now.
}

pub(crate) fn render_graph(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState) {
    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };
    let thread_id = state.summary.id.clone();

    for post in &posts {
        debug!(
            "graph node post={} parents={:?} body_len={}",
            post.id,
            post.parent_post_ids,
            post.body.len()
        );
    }

    // Initialize graph if empty
    if state.graph_nodes.is_empty() || state.graph_nodes.len() != posts.len() {
        state.graph_nodes = build_initial_graph(&posts);
        state.sim_start_time = Some(Instant::now());
    }

    // Update node sizes first so physics knows about them
    for post in &posts {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            let attachments = state.attachments.get(&post.id).cloned();
            let has_preview = attachments
                .as_ref()
                .map(|files| files.iter().any(is_image))
                .unwrap_or(false);
            let height = estimate_node_height(ui, post, has_preview);
            node.size = egui::vec2(320.0, height);
        }
    }

    let scale = 100.0; // 1 unit = 100 pixels

    // Run simulation for a bit longer initially, or continuously if needed
    let sim_active = state.sim_start_time
        .get_or_insert_with(Instant::now)
        .elapsed()
        .as_secs_f32() < 5.0;

    if sim_active && !state.graph_dragging && !state.sim_paused {
        for _ in 0..10 { // Run more steps per frame for smoother/faster expansion
            step_graph_layout(&mut state.graph_nodes, &posts, scale, &thread_id, state.repulsion_force);
        }
        ui.ctx().request_repaint(); // Keep animating
    }

    let available = ui.available_size();
    let (rect, response) = ui.allocate_at_least(available, egui::Sense::click_and_drag());
    let canvas = ui.painter().with_clip_rect(rect);
    canvas.rect_filled(rect, 0.0, Color32::from_rgb(12, 13, 20));
    
    // Grid pattern
    let grid_size = 50.0 * state.graph_zoom;
    let grid_color = Color32::from_rgba_premultiplied(70, 72, 95, 40);
    // Calculate grid offset based on camera
    let center = rect.center();
    let offset = state.graph_offset;
    let zoom = state.graph_zoom * scale; 
    
    // Draw grid
    if grid_size > 5.0 {
        let mut x = (center.x + offset.x) % grid_size;
        if x < rect.left() { x += grid_size; }
        while x < rect.right() {
            canvas.line_segment([egui::pos2(x, rect.top()), egui::pos2(x, rect.bottom())], egui::Stroke::new(1.0, grid_color));
            x += grid_size;
        }
        let mut y = (center.y + offset.y) % grid_size;
        if y < rect.top() { y += grid_size; }
        while y < rect.bottom() {
            canvas.line_segment([egui::pos2(rect.left(), y), egui::pos2(rect.right(), y)], egui::Stroke::new(1.0, grid_color));
            y += grid_size;
        }
    }

    let edge_painter = ui.painter().with_clip_rect(rect);

    // Handle Zoom
    if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pointer_pos) {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let old_zoom = state.graph_zoom;
                let zoom_factor = (1.0 + scroll * 0.001).clamp(0.5, 3.5);
                state.graph_zoom = (old_zoom * zoom_factor).clamp(0.1, 5.0);
                
                // Zoom towards mouse
                let pointer_rel = pointer_pos - center - state.graph_offset;
                let zoom_ratio = state.graph_zoom / old_zoom;
                state.graph_offset += pointer_rel - pointer_rel * zoom_ratio;
            }
        }
    }

    if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
        state.graph_offset += response.drag_delta();
    }

    let world_to_screen = move |p: egui::Pos2| {
        center + offset + egui::vec2(p.x * zoom, p.y * zoom)
    };
    
    let screen_to_world = move |p: egui::Pos2| {
        let rel = p - center - offset;
        egui::pos2(rel.x / zoom, rel.y / zoom)
    };

    // Compute replies map
    let mut replies_map: HashMap<String, Vec<String>> = HashMap::new();
    for post in &posts {
        for parent in &post.parent_post_ids {
            replies_map.entry(parent.clone()).or_default().push(post.id.clone());
        }
    }

    // Draw edges first (behind nodes)
    draw_edges(&edge_painter, &posts, state, &world_to_screen, state.graph_zoom, scale);

    let mut layouts = Vec::new();
    for post in posts.iter() {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            let attachments = state.attachments.get(&post.id).cloned();
            let center_pos = world_to_screen(node.pos);
            // Node pos is center, rect is top-left
            let size = node.size * state.graph_zoom; 
            
            let top_left = center_pos - size / 2.0;
            let rect_node = egui::Rect::from_min_size(top_left, size);
            
            // Only render if visible
            if rect.intersects(rect_node) {
                layouts.push(NodeLayoutData {
                    post: post.clone(),
                    rect: rect_node,
                    attachments,
                });
            }
        }
    }

    // Sort layouts so selected post is drawn last (on top)
    layouts.sort_by_key(|l| state.selected_post.as_ref() == Some(&l.post.id));

    // let rect_lookup: HashMap<String, egui::Rect> = layouts
    //     .iter()
    //     .map(|layout| (layout.post.id.clone(), layout.rect))
    //     .collect();

    // draw_edges(&edge_painter, &layouts, &rect_lookup, state);

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
                        let world_pos = screen_to_world(pos);
                        node.pos = world_pos;
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
                // ui.set_clip_rect(rect_node); // REMOVED CLIPPING
                egui::Frame::none()
                    .fill(fill_color)
                    .stroke(egui::Stroke::new(1.5 * state.graph_zoom, stroke_color))
                    .rounding(egui::Rounding::same(10.0 * state.graph_zoom))
                    .inner_margin(Margin::same(10.0 * state.graph_zoom))
                    .show(ui, |ui| {
                        ui.vertical(|ui| {
                            render_node_header(ui, state, &layout.post, state.graph_zoom);

                            if !layout.post.parent_post_ids.is_empty() {
                                ui.add_space(4.0 * state.graph_zoom);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Replying to:").size(11.0 * state.graph_zoom));
                                    for parent in &layout.post.parent_post_ids {
                                        if ui.add(egui::Link::new(RichText::new(format!("#{parent}")).size(11.0 * state.graph_zoom))).clicked() {
                                            state.selected_post = Some(parent.clone());
                                        }
                                    }
                                });
                            }

                            ui.add_space(6.0 * state.graph_zoom);
                            ui.add(
                                egui::Label::new(
                                    egui::RichText::new(&layout.post.body)
                                        .size(13.0 * state.graph_zoom)
                                        .color(Color32::from_rgb(220, 220, 230)),
                                )
                                .wrap(true),
                            );

                            render_node_attachments(
                                app,
                                ui,
                                layout.attachments.as_ref(),
                                &api_base,
                                state.graph_zoom,
                            );

                            ui.add_space(6.0 * state.graph_zoom);
                            render_node_actions(ui, state, &layout.post, state.graph_zoom);
                            
                            // Render replies
                            if let Some(replies) = replies_map.get(&layout.post.id) {
                                ui.add_space(4.0 * state.graph_zoom);
                                ui.separator();
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("Replies:").size(10.0 * state.graph_zoom).color(Color32::GRAY));
                                    for reply_id in replies {
                                        if ui.add(egui::Link::new(RichText::new(format!(">>{reply_id}")).size(10.0 * state.graph_zoom))).clicked() {
                                            state.selected_post = Some(reply_id.clone());
                                        }
                                    }
                                });
                            }

                            // Pin Button
                            ui.add_space(2.0 * state.graph_zoom);
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                let is_pinned = state.graph_nodes.get(&layout.post.id).map(|n| n.pinned).unwrap_or(false);
                                let text = if is_pinned { "📌" } else { "📍" };
                                let btn = egui::Button::new(egui::RichText::new(text).size(14.0 * state.graph_zoom))
                                    .frame(false);
                                let resp = ui.add(btn.sense(egui::Sense::click_and_drag()));
                                
                                if resp.clicked() {
                                    if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                                        node.pinned = !node.pinned;
                                    }
                                }
                                
                                if resp.drag_started() {
                                    if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                                        node.dragging = true;
                                    }
                                    state.graph_dragging = true;
                                }
                                if resp.drag_stopped() {
                                    if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                                        node.dragging = false;
                                    }
                                    state.graph_dragging = false;
                                }
                            });
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
        
        if combined_drag.clicked() {
            state.selected_post = Some(layout.post.id.clone());
        }
    }

    // Controls overlay
    let control_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(10.0, rect.height() - 40.0),
        egui::vec2(rect.width() - 20.0, 30.0)
    );
    
    ui.allocate_ui_at_rect(control_rect, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!(
                "Zoom: {:.2} | Nodes: {}",
                state.graph_zoom,
                state.graph_nodes.len()
            ));
            
            ui.separator();
            
            ui.label("Repulsion:");
            ui.add(egui::Slider::new(&mut state.repulsion_force, 0.0..=2000.0).text(""));

            ui.separator();
            
            let icon = if state.sim_paused { "▶" } else { "⏸" };
            if ui.button(icon).clicked() {
                state.sim_paused = !state.sim_paused;
            }

            if ui.button("Reset Layout").clicked() {
                state.graph_nodes = build_initial_graph(&posts);
                state.sim_start_time = Some(Instant::now());
                state.graph_offset = egui::vec2(0.0, 0.0);
                state.graph_zoom = 1.0;
            }
        });
    });
}

fn render_node_header(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView, zoom: f32) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!("#{}", post.id))
                .color(Color32::from_rgb(150, 160, 180))
                .size(11.0 * zoom),
        );
        if let Some(name) = &post.author_peer_id {
            ui.label(
                egui::RichText::new(name)
                    .color(Color32::from_rgb(100, 200, 100))
                    .size(11.0 * zoom)
                    .strong(),
            );
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format_timestamp(&post.created_at))
                    .color(Color32::from_rgb(100, 100, 120))
                    .size(10.0 * zoom),
            );
        });
    });
}

fn render_node_actions(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView, zoom: f32) {
    ui.horizontal(|ui| {
        let btn_text_size = 12.0 * zoom;
        if ui.add(egui::Button::new(egui::RichText::new("↩ Reply").size(btn_text_size))).clicked() {
            state.reply_to = vec![post.id.clone()];
        }
        if ui.add(egui::Button::new(egui::RichText::new("📋 Quote").size(btn_text_size))).clicked() {
            state.new_post_body.push_str(&format!(">>{}\n", post.id));
        }
    });
}

fn render_node_attachments(
    app: &mut GraphchanApp,
    ui: &mut egui::Ui,
    attachments: Option<&Vec<FileResponse>>,
    api_base: &str,
    zoom: f32,
) {
    if let Some(files) = attachments {
        for file in files {
            if is_image(file) {
                let url = format!("{}{}", api_base, file.path);
                ui.add_space(4.0 * zoom);
                
                // Load texture if needed
                if !app.image_textures.contains_key(&file.id) && !app.image_loading.contains(&file.id) {
                    app.spawn_download_image(&file.id, &url);
                }

                if let Some(texture) = app.image_textures.get(&file.id) {
                    let size = texture.size_vec2();
                    let max_width = 300.0 * zoom;
                    let scale = if size.x > max_width {
                        max_width / size.x
                    } else {
                        1.0
                    };
                    let display_size = size * scale;
                    
                    let resp = ui.add(egui::Image::from_texture(texture).fit_to_exact_size(display_size).sense(egui::Sense::click()));
                    if resp.clicked() {
                        app.image_viewers.insert(file.id.clone(), true);
                    }
                } else {
                    ui.label(egui::RichText::new("Loading image...").size(11.0 * zoom));
                }
            } else {
                let name = file.original_name.as_deref().unwrap_or("file");
                ui.hyperlink_to(
                    egui::RichText::new(name).size(11.0 * zoom),
                    format!("{}{}", api_base, file.path)
                );
            }
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
    let ext = std::path::Path::new(&file.path)
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();
    matches!(ext.as_str(), "jpg" | "jpeg" | "png" | "gif" | "webp")
}

pub(super) enum ImagePreview {
    Ready(egui::TextureHandle),
    Loading,
    Error(String),
    None,
}

fn draw_edges(
    painter: &egui::Painter,
    posts: &[PostView],
    state: &ThreadState,
    world_to_screen: impl Fn(egui::Pos2) -> egui::Pos2,
    zoom: f32,
    _scale: f32,
) {
    let reply_targets = &state.reply_to;
    let nodes = &state.graph_nodes;

    for post in posts {
        for parent_id in &post.parent_post_ids {
            let (parent_node, child_node) = match (nodes.get(parent_id), nodes.get(&post.id)) {
                (Some(p), Some(c)) => (p, c),
                _ => continue,
            };

            let p_pos = world_to_screen(parent_node.pos);
            let c_pos = world_to_screen(child_node.pos);
            
            // Calculate anchor points based on size and zoom
            let p_size = parent_node.size * zoom;
            let c_size = child_node.size * zoom;
            
            let start = pos_with_offset(p_pos + egui::vec2(0.0, p_size.y / 2.0), 0.0, 6.0 * zoom);
            let end = pos_with_offset(c_pos - egui::vec2(0.0, c_size.y / 2.0), 0.0, -6.0 * zoom);

            let is_reply_edge = reply_targets
                .iter()
                .any(|id| id == parent_id || id == &post.id);
            let sel = state.selected_post.as_ref();
            let color = if is_reply_edge {
                Color32::from_rgb(255, 190, 92)
            } else if sel == Some(&post.id) || sel == Some(parent_id) {
                Color32::from_rgb(110, 190, 255)
            } else {
                Color32::from_rgb(90, 110, 170)
            };

            // Only draw if at least one end is somewhat visible or they cross the screen
            // For now, just draw everything, the clipper handles the actual geometry clipping usually
            // But we can optimize if needed.
            
            painter.line_segment(
                [start, end],
                egui::Stroke::new(if is_reply_edge { 3.4 * zoom } else { 2.0 * zoom }, color),
            );

            draw_arrow(painter, start, end, color, zoom);
        }
    }
}

fn pos_with_offset(mut pos: Pos2, dx: f32, dy: f32) -> Pos2 {
    pos.x += dx;
    pos.y += dy;
    pos
}

fn draw_arrow(painter: &egui::Painter, start: Pos2, end: Pos2, color: Color32, zoom: f32) {
    let dir = (end - start).normalized();
    let normal = egui::Vec2::new(-dir.y, dir.x);
    let arrow_size = 8.0 * zoom;
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


