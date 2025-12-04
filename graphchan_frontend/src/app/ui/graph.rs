use std::collections::HashMap;

use std::time::Instant;

use eframe::egui::{self, Color32};
use log::debug;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::models::{PostView};

use super::super::state::{GraphNode, ThreadState};
use super::super::{GraphchanApp};
use super::node::{render_node, estimate_node_size, is_image, NodeLayoutData};
use super::input;


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

fn step_graph_layout(
    nodes: &mut HashMap<String, GraphNode>,
    ids: &[String],
    edges: &[(String, String)],
    _scale: f32,
    thread_id: &str,
    repulsion_force: f32,
    desired_edge_length: f32,
) {
    let repulsion = repulsion_force / 1000.0; // Scale down for physics
    let attraction = 0.015; // Increased from 0.002 to keep things tighter
    let damping = 0.80; // Increased damping for stability
    let desired = desired_edge_length;
    
    // 1. Apply Forces
    for i in 0..ids.len() {
        // Skip OP forces
        if ids[i] == *thread_id { continue; }
        
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
            let na = nodes.get(a).unwrap();
            let nb = nodes.get(b).unwrap();
            (na.pos, nb.pos)
        };
        let delta = pb - pa;
        let dist = delta.length().max(0.0001);
        let dir = delta / dist;
        
        // Spring attraction
        let force = attraction * (dist - desired);
        
        if a != thread_id { // Keep thread_id check for edges? No, use pinned.
            if let Some(node) = nodes.get_mut(a) {
                if !node.pinned {
                    node.vel += dir * force;
                }
            }
        }
        if b != thread_id {
            if let Some(node) = nodes.get_mut(b) {
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
}

pub(crate) fn render_graph(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState) {
    // Keyboard input is now handled in app/mod.rs update()
    // input::handle_keyboard_input(app, ui);

    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };
    let thread_id = state.summary.id.clone();



    // Initialize graph if empty
    if state.graph_nodes.is_empty() || state.graph_nodes.len() != posts.len() {
        state.graph_nodes = build_initial_graph(&posts);
        state.sim_start_time = Some(Instant::now());
    }

    // Pre-calculate children for each node
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for post in &posts {
        for parent_id in &post.parent_post_ids {
            children_map.entry(parent_id.clone()).or_default().push(post.id.clone());
        }
    }

    // Update node sizes first so physics knows about them
    for post in &posts {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            // Prefer post.files over state.attachments (post.files has correct present flag)
            let attachments = if !post.files.is_empty() {
                Some(post.files.clone())
            } else {
                state.attachments.get(&post.id).cloned()
            };
            let has_preview = attachments
                .as_ref()
                .map(|files| files.iter().any(is_image))
                .unwrap_or(false);
            let children_count = children_map.get(&post.id).map(|c| c.len()).unwrap_or(0);
            
            // Using 1.0 zoom for physics size calculation
            node.size = estimate_node_size(ui, post, has_preview, children_count);
        }
    }

    let scale = 100.0; // 1 unit = 100 pixels
    
    // Pre-calculate simulation data once per frame
    let ids: Vec<String> = state.graph_nodes.keys().cloned().collect();
    let mut edges = Vec::new();
    for post in &posts {
        for parent in &post.parent_post_ids {
            if state.graph_nodes.contains_key(parent) {
                edges.push((parent.clone(), post.id.clone()));
            }
        }
    }

    // Run simulation for a bit longer initially, or continuously if needed
    let sim_active = state.sim_start_time
        .get_or_insert_with(Instant::now)
        .elapsed()
        .as_secs_f32() < 5.0;

    if sim_active && !state.graph_dragging && !state.sim_paused {
        for _ in 0..2 { // Reduced from 10 to 2 for performance
            step_graph_layout(&mut state.graph_nodes, &ids, &edges, scale, &thread_id, state.repulsion_force, state.desired_edge_length);
        }
        ui.ctx().request_repaint(); // Keep animating
    } else if state.graph_dragging {
        ui.ctx().request_repaint(); // Keep animating during drag
    }

    let available = ui.available_size();
    let (rect, response) = ui.allocate_at_least(available, egui::Sense::click_and_drag());

    let canvas = ui.painter().with_clip_rect(rect);
    canvas.rect_filled(rect, 0.0, Color32::from_rgb(12, 13, 20));
    
    // Dot grid pattern
    let dot_spacing = 50.0 * state.graph_zoom;
    let dot_color = Color32::from_rgba_premultiplied(70, 72, 95, 60);
    
    // Calculate grid offset based on camera
    let center = rect.center();
    let offset = state.graph_offset;
    let zoom = state.graph_zoom * scale;
    
    // Draw dots
    if dot_spacing > 10.0 {
        let mut x = (center.x + offset.x) % dot_spacing;
        if x < rect.left() { x += dot_spacing; }
        while x < rect.right() {
            let mut y = (center.y + offset.y) % dot_spacing;
            if y < rect.top() { y += dot_spacing; }
            while y < rect.bottom() {
                canvas.circle_filled(egui::pos2(x, y), 1.5, dot_color);
                y += dot_spacing;
            }
            x += dot_spacing;
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
                state.graph_zoom = (old_zoom * zoom_factor).clamp(0.05, 5.0);
                
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
    
    if response.clicked() {
        state.selected_post = None;
    }

    let world_to_screen = move |p: egui::Pos2| {
        center + offset + egui::vec2(p.x * zoom, p.y * zoom)
    };
    
    let screen_to_world = move |p: egui::Pos2| {
        let rel = p - center - offset;
        egui::pos2(rel.x / zoom, rel.y / zoom)
    };

    // Draw edges first (behind nodes)
    draw_edges(&edge_painter, &posts, state, &world_to_screen, state.graph_zoom, scale);

    let mut layouts = Vec::new();
    for post in posts.iter() {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            // Prefer post.files over state.attachments (post.files has correct present flag)
            let attachments = if !post.files.is_empty() {
                Some(post.files.clone())
            } else {
                state.attachments.get(&post.id).cloned()
            };
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

    let api_base = app.api.base_url().to_string();

    for layout in layouts {
        let children = children_map.get(&layout.post.id).cloned().unwrap_or_default();
        
        // Interaction handled here to ensure it works
        let drag_id = ui.make_persistent_id(format!("graph_node_drag_{}", layout.post.id));
        let drag_handle = ui.interact(layout.rect, drag_id, egui::Sense::click_and_drag());

        if drag_handle.drag_started() {
             if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                node.dragging = true;
            }
            state.graph_dragging = true;
        }
        
        if drag_handle.dragged() {
            if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                if node.dragging {
                    let zoom_scale = state.graph_zoom * scale;
                    node.pos += drag_handle.drag_delta() / zoom_scale;
                }
            }
        }
        
        if drag_handle.drag_stopped() {
            if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                node.dragging = false;
            }
            state.graph_dragging = false;
        }

        if drag_handle.clicked() {
            state.selected_post = Some(layout.post.id.clone());
        }

        let is_neighbor = if let Some(sel_id) = &state.selected_post {
            if sel_id == &layout.post.id {
                false // Selected is handled separately
            } else {
                // Check if neighbor
                // Parent of selected?
                let is_parent_of_selected = state.details.as_ref()
                    .and_then(|d| d.posts.iter().find(|p| p.id == *sel_id))
                    .map(|p| p.parent_post_ids.contains(&layout.post.id))
                    .unwrap_or(false);
                
                // Child of selected?
                let is_child_of_selected = layout.post.parent_post_ids.contains(sel_id);
                
                is_parent_of_selected || is_child_of_selected
            }
        } else {
            false
        };

        let is_secondary = state.secondary_selected_post.as_ref() == Some(&layout.post.id);

        let _node_response = render_node(
            app,
            ui,
            state,
            &layout,
            &api_base,
            rect,
            state.graph_zoom,
            &children,
            is_neighbor,
            is_secondary
        );
        
        // Pin Button Overlay
        let pin_rect = egui::Rect::from_min_size(
            layout.rect.max - egui::vec2(20.0 * state.graph_zoom, 20.0 * state.graph_zoom),
            egui::vec2(20.0 * state.graph_zoom, 20.0 * state.graph_zoom)
        );
        
        // We need to allocate this UI on top
        ui.allocate_ui_at_rect(pin_rect, |ui| {
            let is_pinned = state.graph_nodes.get(&layout.post.id).map(|n| n.pinned).unwrap_or(false);
            let text = if is_pinned { "📌" } else { "📍" };
            if ui.add(egui::Button::new(egui::RichText::new(text).size(12.0 * state.graph_zoom)).frame(false)).clicked() {
                if let Some(node) = state.graph_nodes.get_mut(&layout.post.id) {
                    node.pinned = !node.pinned;
                }
            }
        });
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

            ui.label("Edge Length:");
            ui.add(egui::Slider::new(&mut state.desired_edge_length, 0.5..=20.0).text(""));

            ui.separator();
            
            let sim_active = state.sim_start_time
                .get_or_insert_with(Instant::now)
                .elapsed()
                .as_secs_f32() < 5.0;
            let running = sim_active && !state.sim_paused && !state.graph_dragging;

            let icon = if running { "⏸" } else { "▶" };
            if ui.button(icon).clicked() {
                if running {
                    state.sim_paused = true;
                } else {
                    state.sim_paused = false;
                    state.sim_start_time = Some(Instant::now());
                }
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
            let is_selected_edge = sel == Some(&post.id) || sel == Some(parent_id);
            
            let color = if is_reply_edge {
                Color32::from_rgb(255, 190, 92)
            } else if is_selected_edge {
                Color32::from_rgb(250, 208, 108) // Yellow for selected edges
            } else {
                Color32::from_rgb(90, 110, 170)
            };
            
            let stroke_width = if is_reply_edge || is_selected_edge { 3.4 * zoom } else { 2.0 * zoom };
            
            painter.line_segment(
                [start, end],
                egui::Stroke::new(stroke_width, color),
            );

            draw_arrow(painter, start, end, color, zoom);
        }
    }
}

fn pos_with_offset(mut pos: egui::Pos2, dx: f32, dy: f32) -> egui::Pos2 {
    pos.x += dx;
    pos.y += dy;
    pos
}

fn draw_arrow(painter: &egui::Painter, start: egui::Pos2, end: egui::Pos2, color: Color32, zoom: f32) {
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
