use std::collections::HashMap;
use eframe::egui::{self, Color32};
use crate::models::PostView;
use super::super::state::{GraphNode, ThreadState};
use super::super::GraphchanApp;
use super::node::{render_node, NodeLayoutData};

const NODE_WIDTH: f32 = 300.0;
const LAYER_HEIGHT: f32 = 200.0;
const NODE_SPACING: f32 = 50.0;

pub fn build_sugiyama_layout(posts: &[PostView]) -> HashMap<String, GraphNode> {
    if posts.is_empty() {
        return HashMap::new();
    }

    let mut nodes = HashMap::new();
    
    // 1. Assign Ranks (Depth)
    let mut ranks: HashMap<String, usize> = HashMap::new();
    let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();
    let mut reverse_adjacency: HashMap<String, Vec<String>> = HashMap::new();
    
    for post in posts {
        adjacency.entry(post.id.clone()).or_default();
        for parent in &post.parent_post_ids {
            adjacency.entry(parent.clone()).or_default().push(post.id.clone());
            reverse_adjacency.entry(post.id.clone()).or_default().push(parent.clone());
        }
    }
    
    // BFS for ranking
    let mut queue = std::collections::VecDeque::new();
    
    // Find roots
    for post in posts {
        if post.parent_post_ids.is_empty() {
            ranks.insert(post.id.clone(), 0);
            queue.push_back(post.id.clone());
        }
    }
    
    // If no roots, pick oldest
    if queue.is_empty() {
         if let Some(oldest) = posts.iter().min_by_key(|p| &p.created_at) {
            ranks.insert(oldest.id.clone(), 0);
            queue.push_back(oldest.id.clone());
        }
    }
    
    while let Some(node_id) = queue.pop_front() {
        let current_rank = *ranks.get(&node_id).unwrap();
        if let Some(children) = adjacency.get(&node_id) {
            for child in children {
                if !ranks.contains_key(child) {
                    ranks.insert(child.clone(), current_rank + 1);
                    queue.push_back(child.clone());
                }
            }
        }
    }
    
    // Group by rank
    let mut layers: HashMap<usize, Vec<String>> = HashMap::new();
    for (id, rank) in ranks {
        layers.entry(rank).or_default().push(id);
    }
    
    // 2. Ordering (Simple heuristic: keep order of parents)
    let max_rank = *layers.keys().max().unwrap_or(&0);
    
    for r in 1..=max_rank {
        // Clone previous layer to avoid borrow issues
        let prev_layer = layers.get(&(r-1)).cloned().unwrap_or_default();
        
        if let Some(layer_nodes) = layers.get_mut(&r) {
            layer_nodes.sort_by_cached_key(|node_id| {
                // Calculate average parent index in previous layer
                let parents = reverse_adjacency.get(node_id).cloned().unwrap_or_default();
                if parents.is_empty() { return 0; }
                
                let sum: usize = parents.iter()
                    .filter_map(|pid| prev_layer.iter().position(|x| x == pid))
                    .sum();
                sum / parents.len().max(1)
            });
        }
    }
    
    // 3. Coordinate Assignment
    for (rank, layer_nodes) in layers {
        let layer_width = layer_nodes.len() as f32 * (NODE_WIDTH + NODE_SPACING);
        let start_x = -layer_width / 2.0;
        
        for (i, node_id) in layer_nodes.iter().enumerate() {
            let x = start_x + i as f32 * (NODE_WIDTH + NODE_SPACING);
            let y = rank as f32 * LAYER_HEIGHT;
            
            nodes.insert(node_id.clone(), GraphNode {
                pos: egui::pos2(x, y),
                vel: egui::vec2(0.0, 0.0),
                size: egui::vec2(NODE_WIDTH, 150.0),
                dragging: false,
                pinned: false,
            });
        }
    }

    nodes
}

pub fn render_sugiyama(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState) {
    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };

    if state.sugiyama_nodes.is_empty() || state.sugiyama_nodes.len() != posts.len() {
        state.sugiyama_nodes = build_sugiyama_layout(&posts);
    }

    let available_rect = ui.available_rect_before_wrap();
    let response = ui.allocate_rect(available_rect, egui::Sense::click_and_drag());
    let rect = available_rect;
    let canvas = ui.painter().with_clip_rect(rect);

    canvas.rect_filled(rect, 0.0, Color32::from_rgb(15, 12, 15));

    // Zoom/Pan
    let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
    let zoom_delta = ui.input(|i| i.zoom_delta());
    let mut total_zoom_factor = 1.0;
    if scroll_delta != 0.0 { total_zoom_factor *= 1.0 + scroll_delta * 0.001; }
    if zoom_delta != 1.0 { total_zoom_factor *= zoom_delta; }
    if total_zoom_factor != 1.0 {
        let old_zoom = state.graph_zoom;
        state.graph_zoom = (state.graph_zoom * total_zoom_factor).clamp(0.01, 10.0);
        if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
            if rect.contains(pointer_pos) {
                let pointer_rel = pointer_pos - rect.min;
                let zoom_ratio = state.graph_zoom / old_zoom;
                state.graph_offset = pointer_rel - (pointer_rel - state.graph_offset) * zoom_ratio;
            }
        }
    }
    if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
        state.graph_offset += response.drag_delta();
    }

    let center_offset = rect.center().to_vec2() + state.graph_offset;

    // Layouts
    let mut layouts = Vec::new();
    for post in &posts {
        if let Some(node) = state.sugiyama_nodes.get(&post.id) {
            let screen_pos = rect.min + center_offset + (node.pos.to_vec2() * state.graph_zoom);
            let width = NODE_WIDTH * state.graph_zoom;
            let height = 120.0 * state.graph_zoom;
            
            let rect_node = egui::Rect::from_min_size(screen_pos, egui::vec2(width, height));
            
            layouts.push(NodeLayoutData {
                post: post.clone(),
                rect: rect_node,
                attachments: if !post.files.is_empty() { Some(post.files.clone()) } else { None },
            });
        }
    }

    // Edges (Curved Bezier)
    for layout in &layouts {
        for parent_id in &layout.post.parent_post_ids {
            if let Some(parent_layout) = layouts.iter().find(|l| &l.post.id == parent_id) {
                let start = parent_layout.rect.center_bottom();
                let end = layout.rect.center_top();
                
                let control1 = start + egui::vec2(0.0, 50.0 * state.graph_zoom);
                let control2 = end - egui::vec2(0.0, 50.0 * state.graph_zoom);
                
                let cubic = egui::epaint::CubicBezierShape::from_points_stroke(
                    [start, control1, control2, end],
                    false,
                    Color32::TRANSPARENT,
                    egui::Stroke::new(1.5 * state.graph_zoom, Color32::from_rgb(100, 150, 200))
                );
                canvas.add(cubic);
            }
        }
    }

    // Nodes
    let api_base = app.api.base_url().to_string();
    
    // Pre-calculate children for each node for the renderer
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for post in posts {
        for parent_id in &post.parent_post_ids {
            children_map.entry(parent_id.clone()).or_default().push(post.id.clone());
        }
    }

    for layout in layouts {
        let children = children_map.get(&layout.post.id).cloned().unwrap_or_default();
        render_node(
            app,
            ui,
            state,
            &layout,
            &api_base,
            rect,
            state.graph_zoom,
            &children
        );
    }
    
    // Controls
    ui.allocate_ui_at_rect(rect, |ui| {
        ui.horizontal(|ui| {
            ui.label("Hierarchical View");
            if ui.button("Reset").clicked() {
                state.graph_offset = egui::vec2(0.0, 0.0);
                state.graph_zoom = 0.8;
            }
        });
    });
}
