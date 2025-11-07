use std::collections::HashMap;
use std::time::Instant;

use eframe::egui::{self, Align2, Color32, RichText};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use crate::models::PostView;

use super::super::format_timestamp;
use super::super::state::{GraphNode, ThreadState};

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

pub(crate) fn render_graph(ui: &mut egui::Ui, state: &mut ThreadState) {
    let posts = match &state.details {
        Some(d) => &d.posts,
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
            step_graph_layout(&mut state.graph_nodes, posts);
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

    let world_to_screen = |p: egui::Pos2| {
        egui::pos2(
            rect.left() + state.graph_offset.x + p.x * rect.width() * state.graph_zoom,
            rect.top() + state.graph_offset.y + p.y * rect.height() * state.graph_zoom,
        )
    };

    for post in posts {
        for parent in &post.parent_post_ids {
            if let (Some(a), Some(b)) = (
                state.graph_nodes.get(parent),
                state.graph_nodes.get(&post.id),
            ) {
                let pa = world_to_screen(a.pos);
                let pb = world_to_screen(b.pos);
                let sel = state.selected_post.as_ref();
                let color = if sel == Some(&post.id) || sel == Some(parent) {
                    Color32::from_rgb(255, 215, 0)
                } else {
                    Color32::GRAY
                };
                painter.line_segment(
                    [pa, pb],
                    egui::Stroke {
                        width: if color == Color32::GRAY { 1.0 } else { 2.0 },
                        color,
                    },
                );
            } else if state.graph_nodes.get(parent).is_none() {
                let placeholder = GraphNode {
                    pos: egui::pos2(0.05, 0.05),
                    vel: egui::vec2(0.0, 0.0),
                    id: parent.clone(),
                    size: egui::vec2(120.0, 80.0),
                    dragging: false,
                };
                state.graph_nodes.insert(parent.clone(), placeholder);
            }
        }
    }

    let mut clicked: Option<String> = None;
    let mut drag_target: Option<String> = None;
    for post in posts {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            let top_left = world_to_screen(node.pos);
            let size = node.size * state.graph_zoom;
            let rect_node = egui::Rect::from_min_size(top_left, size);
            let hovered = rect_node.contains(response.hover_pos().unwrap_or_default());
            let selected = state.selected_post.as_ref() == Some(&post.id);
            let fill = if selected {
                Color32::from_rgb(60, 60, 20)
            } else if hovered {
                Color32::from_rgb(50, 50, 60)
            } else {
                ui.visuals().extreme_bg_color
            };
            painter.rect_filled(rect_node, 6.0, fill);
            painter.rect_stroke(
                rect_node,
                6.0,
                egui::Stroke {
                    width: 1.5,
                    color: if selected {
                        Color32::YELLOW
                    } else {
                        Color32::from_rgb(100, 150, 220)
                    },
                },
            );
            let preview = post.body.lines().take(4).collect::<Vec<_>>().join("\n");
            let text = &preview[..preview.len().min(120)];
            painter.text(
                rect_node.min + egui::vec2(8.0, 8.0),
                Align2::LEFT_TOP,
                text,
                egui::FontId::proportional(12.0),
                Color32::WHITE,
            );
            if hovered && response.clicked() {
                clicked = Some(post.id.clone());
            }
            if hovered && response.dragged_by(egui::PointerButton::Primary) {
                drag_target = Some(post.id.clone());
            }
            if node.dragging && response.drag_stopped() {
                node.dragging = false;
                state.graph_dragging = false;
            }
        }
    }

    if let Some(id) = drag_target {
        if let Some(node) = state.graph_nodes.get_mut(&id) {
            node.dragging = true;
            state.graph_dragging = true;
        }
    }

    if state.graph_dragging {
        if let Some(pos) = response.hover_pos() {
            for node in state.graph_nodes.values_mut() {
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

    if let Some(id) = clicked {
        state.selected_post = Some(id);
    }

    if let Some(sel) = &state.selected_post {
        if let Some(post) = posts.iter().find(|p| &p.id == sel) {
            ui.separator();
            ui.label(RichText::new(format!("Post {}", post.id)).strong());
            ui.label(format_timestamp(&post.created_at));
            ui.label(&post.body);
            if !post.parent_post_ids.is_empty() {
                ui.label(format!("Parents: {}", post.parent_post_ids.join(", ")));
            }
        }
    }

    ui.separator();
    ui.label(format!(
        "Zoom: {:.2} | Nodes: {}",
        state.graph_zoom,
        state.graph_nodes.len()
    ));
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
