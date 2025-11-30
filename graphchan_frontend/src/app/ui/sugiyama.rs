use std::collections::HashMap;
use eframe::egui::{self, Color32};
use crate::models::PostView;
use super::super::state::{GraphNode, ThreadState};
use super::super::GraphchanApp;
use super::node::{render_node, estimate_node_size, is_image, NodeLayoutData};

const NODE_WIDTH: f32 = 300.0;
const LAYER_HEIGHT: f32 = 200.0;
const NODE_SPACING: f32 = 50.0;

pub fn build_sugiyama_layout(posts: &[PostView], sizes: &HashMap<String, egui::Vec2>) -> HashMap<String, GraphNode> {
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
    let layer_spacing = 100.0;
    let node_spacing = 80.0;
    let mut current_y = 0.0;

    for rank in 0..=max_rank {
        if let Some(layer_nodes) = layers.get(&rank) {
            // Calculate max height for this layer
            let max_h = layer_nodes.iter()
                .map(|id| sizes.get(id).map(|s| s.y).unwrap_or(150.0))
                .fold(0.0f32, |a, b| a.max(b));
            
            let mut layer_width = 0.0;
            for node_id in layer_nodes {
                 let size = sizes.get(node_id).cloned().unwrap_or(egui::vec2(300.0, 150.0));
                 layer_width += size.x + node_spacing;
            }
            if !layer_nodes.is_empty() {
                layer_width -= node_spacing;
            }
    
            let mut current_x = -layer_width / 2.0;
            
            for node_id in layer_nodes.iter() {
                let size = sizes.get(node_id).cloned().unwrap_or(egui::vec2(300.0, 150.0));
                let x = current_x + size.x / 2.0;
                // Align top of nodes to the current_y
                let y = current_y + size.y / 2.0;
                
                nodes.insert(node_id.clone(), GraphNode {
                    pos: egui::pos2(x, y),
                    vel: egui::vec2(0.0, 0.0),
                    size,
                    dragging: false,
                    pinned: false,
                });
                
                current_x += size.x + node_spacing;
            }
            
            current_y += max_h + layer_spacing;
        }
    }

    nodes
}

pub fn render_sugiyama(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState) {
    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };

    // Pre-calculate children for each node
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for post in &posts {
        for parent_id in &post.parent_post_ids {
            children_map.entry(parent_id.clone()).or_default().push(post.id.clone());
        }
    }

    // Calculate sizes for all posts
    let mut sizes = HashMap::new();
    for post in &posts {
        // Prefer post.files over state.attachments
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
            
        let size = estimate_node_size(ui, post, has_preview, children_count);
        sizes.insert(post.id.clone(), size);
    }

    if state.sugiyama_nodes.is_empty() || state.sugiyama_nodes.len() != posts.len() {
        state.sugiyama_nodes = build_sugiyama_layout(&posts, &sizes);
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
                // Zoom towards mouse
                let center = rect.center();
                let pointer_rel = pointer_pos - center - state.graph_offset;
                let zoom_ratio = state.graph_zoom / old_zoom;
                state.graph_offset += pointer_rel - pointer_rel * zoom_ratio;
            }
        }
    }
    
    // Handle panning
    if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
        state.graph_offset += response.drag_delta();
    }
    
    // Clear selection on background click
    if response.clicked() {
        state.selected_post = None;
    }

    // Correct center offset calculation
    let center_offset = rect.center().to_vec2() + state.graph_offset;

    // Layouts
    let mut layouts = Vec::new();
    for post in &posts {
        if let Some(node) = state.sugiyama_nodes.get(&post.id) {
            // node.pos is relative to (0,0) center of the layout
            // We need to map it to screen space: Center + Offset + Pos * Zoom
            // Note: center_offset is (Center + Offset) as Vec2.
            // But rect.min needs to be added if we use absolute coordinates?
            // rect.center() is absolute.
            // So ScreenPos = rect.center() + offset + node.pos * zoom.
            // My center_offset is rect.center().to_vec2() + offset.
            // So ScreenPos = center_offset.to_pos2() + node.pos * zoom.
            // Wait, center_offset is Vec2.
            // Pos2 + Vec2 = Pos2.
            // So (0,0) + center_offset = Center + Offset.
            // ScreenPos = (Pos2::ZERO + center_offset) + node.pos * zoom.
            
            let screen_center = egui::pos2(0.0, 0.0) + center_offset + (node.pos.to_vec2() * state.graph_zoom);
            let size = node.size * state.graph_zoom;
            
            let top_left = screen_center - size / 2.0;
            let rect_node = egui::Rect::from_min_size(top_left, size);
            
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
                
                let is_selected_edge = state.selected_post.as_ref() == Some(&layout.post.id) || state.selected_post.as_ref() == Some(parent_id);
                let color = if is_selected_edge {
                    Color32::from_rgb(250, 208, 108)
                } else {
                    Color32::from_rgb(100, 150, 200)
                };
                let stroke_width = if is_selected_edge { 2.5 * state.graph_zoom } else { 1.5 * state.graph_zoom };

                let cubic = egui::epaint::CubicBezierShape::from_points_stroke(
                    [start, control1, control2, end],
                    false,
                    Color32::TRANSPARENT,
                    egui::Stroke::new(stroke_width, color)
                );
                canvas.add(cubic);
            }
        }
    }

    let api_base = app.api.base_url().to_string();
    
    for layout in layouts {
        let children = children_map.get(&layout.post.id).cloned().unwrap_or_default();
        
        let is_neighbor = if let Some(sel_id) = &state.selected_post {
            if sel_id == &layout.post.id {
                false
            } else {
                let is_parent_of_selected = state.details.as_ref()
                    .and_then(|d| d.posts.iter().find(|p| p.id == *sel_id))
                    .map(|p| p.parent_post_ids.contains(&layout.post.id))
                    .unwrap_or(false);
                let is_child_of_selected = layout.post.parent_post_ids.contains(sel_id);
                is_parent_of_selected || is_child_of_selected
            }
        } else {
            false
        };

        render_node(
            app,
            ui,
            state,
            &layout,
            &api_base,
            rect,
            state.graph_zoom,
            &children,
            is_neighbor
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
