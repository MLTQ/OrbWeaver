use std::collections::HashMap;

use chrono::{DateTime, Timelike, Utc};
use eframe::egui::{self, Color32};
use log;

use crate::models::{PostView};

use super::super::state::{GraphNode, ThreadState};
use super::super::{format_timestamp, GraphchanApp};
use super::node::{render_node, estimate_node_height, is_image, NodeLayoutData};
use super::input;

/// Configuration for chronological layout

const CARD_WIDTH: f32 = 720.0; // Match 4chan width for familiarity
const CARD_HORIZONTAL_SPACING: f32 = 50.0; // More space between cards
const MIN_BIN_VERTICAL_SPACING: f32 = 200.0; // Increased spacing for edge lanes
const LEFT_MARGIN: f32 = 120.0; // Leave room for time axis
const TOP_MARGIN: f32 = 50.0;


struct ChronoBin {
    timestamp: DateTime<Utc>,
    post_ids: Vec<String>,
}

/// Build chronological layout positions for posts
pub fn build_chronological_layout(posts: &[PostView], bin_seconds: i64) -> HashMap<String, GraphNode> {
    if posts.is_empty() {
        return HashMap::new();
    }

    // Parse timestamps and sort posts by time (oldest first)
    let mut timestamped_posts: Vec<(DateTime<Utc>, &PostView)> = posts
        .iter()
        .filter_map(|p| match DateTime::parse_from_rfc3339(&p.created_at) {
            Ok(dt) => {
                let utc_dt = dt.with_timezone(&Utc);
                log::debug!("Post {} timestamp: {}", p.id, utc_dt);
                Some((utc_dt, p))
            }
            Err(e) => {
                log::warn!("Failed to parse timestamp for post {}: {}", p.id, e);
                None
            }
        })
        .collect();
    timestamped_posts.sort_by_key(|(dt, _)| *dt);

    // Group into time bins using the provided bin size
    let bins = create_time_bins(&timestamped_posts, bin_seconds);
    log::debug!(
        "Created {} time bins from {} posts",
        bins.len(),
        posts.len()
    );
    
    // Log bin details
    for (idx, bin) in bins.iter().enumerate() {
        log::debug!("  Bin {}: {} posts at {}", idx, bin.post_ids.len(), bin.timestamp);
    }

    // Assign positions - posts stack horizontally within each bin
    let mut nodes = HashMap::new();
    let mut current_y = TOP_MARGIN;
    
    for (_bin_idx, bin) in bins.iter().enumerate() {
        // Stack posts horizontally (left to right) within this time bin
        for (idx, post_id) in bin.post_ids.iter().enumerate() {
            let x = LEFT_MARGIN + (idx as f32) * (CARD_WIDTH + CARD_HORIZONTAL_SPACING);
            let y = current_y;
            
            nodes.insert(
                post_id.clone(),
                GraphNode {
                    pos: egui::pos2(x, y),
                    vel: egui::vec2(0.0, 0.0),
                    size: egui::vec2(CARD_WIDTH, 250.0),
                    dragging: false,
                    pinned: false,
                },
            );
        }
        
        // Move to next bin (generous vertical spacing for next time bin)
        current_y += 300.0 + MIN_BIN_VERTICAL_SPACING;
    }

    nodes
}

fn create_time_bins(timestamped_posts: &[(DateTime<Utc>, &PostView)], bin_seconds: i64) -> Vec<ChronoBin> {
    if timestamped_posts.is_empty() {
        return Vec::new();
    }

    let mut bins = Vec::new();
    let mut current_bin: Option<ChronoBin> = None;

    for (timestamp, post) in timestamped_posts {
        let bin_timestamp = round_down_to_bin_seconds(*timestamp, bin_seconds);
        log::debug!("Post {} binned to {}", post.id, bin_timestamp);

        match &mut current_bin {
            Some(bin) if bin.timestamp == bin_timestamp => {
                // Add to current bin
                log::debug!(
                    "  -> Adding to existing bin (now {} posts)",
                    bin.post_ids.len() + 1
                );
                bin.post_ids.push(post.id.clone());
            }
            _ => {
                // Start new bin
                if let Some(completed_bin) = current_bin.take() {
                    log::debug!(
                        "  -> Closing bin with {} posts",
                        completed_bin.post_ids.len()
                    );
                    bins.push(completed_bin);
                }
                log::debug!("  -> Starting new bin");
                current_bin = Some(ChronoBin {
                    timestamp: bin_timestamp,
                    post_ids: vec![post.id.clone()],
                });
            }
        }
    }

    if let Some(bin) = current_bin {
        log::debug!("Final bin with {} posts", bin.post_ids.len());
        bins.push(bin);
    }

    bins
}

fn round_down_to_bin_seconds(dt: DateTime<Utc>, bin_seconds: i64) -> DateTime<Utc> {
    let total_seconds = (dt.minute() as i64 * 60) + (dt.second() as i64);
    let binned_seconds = (total_seconds / bin_seconds) * bin_seconds;
    let binned_minutes = (binned_seconds / 60) as u32;
    let binned_secs = (binned_seconds % 60) as u32;

    dt.date_naive()
        .and_hms_opt(dt.hour(), binned_minutes, binned_secs)
        .unwrap()
        .and_local_timezone(Utc)
        .unwrap()
}

/// Render the chronological view
pub fn render_chronological(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState) {
    input::handle_keyboard_input(app, ui);

    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };

    // Build layout if not already done or if we just switched to this mode
    // Also rebuild if post count changed (new posts loaded)
    if state.chronological_nodes.is_empty() || state.chronological_nodes.len() != posts.len() {
        state.chronological_nodes = build_chronological_layout(&posts, state.time_bin_seconds);
    }

    // Calculate layout rects
    let available_rect = ui.available_rect_before_wrap();
    let control_height = 40.0;
    
    let canvas_rect = egui::Rect::from_min_max(
        available_rect.min,
        egui::Pos2::new(available_rect.max.x, available_rect.max.y - control_height)
    );
    
    let control_rect = egui::Rect::from_min_max(
        egui::Pos2::new(available_rect.min.x, available_rect.max.y - control_height),
        available_rect.max
    );

    // Render Controls in the reserved bottom area
    if control_rect.is_positive() {
        ui.allocate_ui_at_rect(control_rect, |ui| {
            ui.push_id("chronological_controls", |ui| {
                ui.horizontal(|ui| {
                    ui.label(format!(
                        "Chronological View | Nodes: {}",
                        state.chronological_nodes.len()
                    ));
                    
                    ui.separator();
                    
                    // Time bin size slider
                    ui.label("Time Bin:");
                    let mut bin_minutes = (state.time_bin_seconds / 60) as f32;
                    let mut rebuild_layout = false;
                    if ui.add(egui::Slider::new(&mut bin_minutes, 1.0..=1440.0)
                        .logarithmic(true)
                        .custom_formatter(|n, _| {
                            if n < 60.0 {
                                format!("{:.0} min", n)
                            } else {
                                format!("{:.1} hr", n / 60.0)
                            }
                        }))
                        .changed() 
                    {
                        let new_bin_seconds = (bin_minutes * 60.0) as i64;
                        if new_bin_seconds != state.time_bin_seconds {
                            state.time_bin_seconds = new_bin_seconds;
                            rebuild_layout = true;
                        }
                    }
                    
                    if rebuild_layout {
                        let posts = state.details.as_ref().map(|d| d.posts.clone()).unwrap_or_default();
                        state.chronological_nodes = build_chronological_layout(&posts, state.time_bin_seconds);
                    }
                    
                    ui.separator();
                    
                    // Zoom controls
                    ui.label("Zoom:");
                    if ui.button("−").clicked() {
                        state.graph_zoom = (state.graph_zoom * 0.8).max(0.2);
                    }
                    ui.label(format!("{:.1}x", state.graph_zoom));
                    if ui.button("+").clicked() {
                        state.graph_zoom = (state.graph_zoom * 1.25).min(4.0);
                    }
                    if ui.button("Reset").clicked() {
                        state.graph_zoom = 1.0;
                        state.graph_offset = egui::vec2(0.0, 0.0);
                    }
                });
            });
        });
    }

    // 1. Render Canvas
    if !canvas_rect.is_positive() {
        return;
    }
    let response = ui.allocate_rect(canvas_rect, egui::Sense::click_and_drag());
    let rect = canvas_rect; // Use the allocated rect
    let canvas = ui.painter().with_clip_rect(rect);

    // Background
    canvas.rect_filled(rect, 0.0, Color32::from_rgb(12, 13, 20));

    // Dot grid pattern
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
    
    // Draw time axis on the left
    draw_time_axis(&canvas, &posts, state, rect);

    // Handle zoom with Scroll Wheel, Ctrl + Scroll, or Pinch
    let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
    let zoom_delta = ui.input(|i| i.zoom_delta());
    
    let mut total_zoom_factor = 1.0;
    
    // Standard scroll wheel zoom
    if scroll_delta != 0.0 {
        total_zoom_factor *= 1.0 + scroll_delta * 0.001;
    }
    
    // Pinch or Ctrl+Scroll zoom
    if zoom_delta != 1.0 {
        total_zoom_factor *= zoom_delta;
    }

    if total_zoom_factor != 1.0 {
        let old_zoom = state.graph_zoom;
        state.graph_zoom = (state.graph_zoom * total_zoom_factor).clamp(0.05, 5.0);
        
        // Zoom towards mouse cursor if hovered
        if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
            if rect.contains(pointer_pos) {
                // Adjust offset to keep the point under the cursor stationary
                let pointer_rel = pointer_pos - rect.min;
                let zoom_ratio = state.graph_zoom / old_zoom;
                state.graph_offset = pointer_rel - (pointer_rel - state.graph_offset) * zoom_ratio;
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

    // Pre-calculate children for all posts
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for post in &posts {
        for parent_id in &post.parent_post_ids {
            children_map.entry(parent_id.clone()).or_default().push(post.id.clone());
        }
    }

    // Build layouts from cached state.chronological_nodes
    let mut layouts = Vec::new();
    
    for post in &posts {
        if let Some(node) = state.chronological_nodes.get(&post.id) {
            // Prefer post.files over state.attachments (post.files has correct present flag)
            let attachments = if !post.files.is_empty() {
                Some(post.files.clone())
            } else {
                state.attachments.get(&post.id).cloned()
            };
            
            // Calculate screen position
            // ScreenPos = RectMin + Offset + NodePos * Zoom
            let top_left = egui::pos2(
                rect.left() + state.graph_offset.x + node.pos.x * state.graph_zoom,
                rect.top() + state.graph_offset.y + node.pos.y * state.graph_zoom,
            );
            let scaled_size = node.size * state.graph_zoom;
            let rect_node = egui::Rect::from_min_size(top_left, scaled_size);

            layouts.push(NodeLayoutData {
                post: post.clone(),
                rect: rect_node,
                attachments,
            });
        }
    }

    // Build lookup for edge routing
    let rect_lookup: HashMap<String, egui::Rect> = layouts
        .iter()
        .map(|layout| (layout.post.id.clone(), layout.rect))
        .collect();

    // Detect hovered post
    let hovered_post = ui.ctx().pointer_hover_pos().and_then(|pos| {
        layouts.iter()
            .find(|layout| layout.rect.contains(pos))
            .map(|layout| layout.post.id.clone())
    });

    // Draw orthogonal edges (clipped to viewport)
    let edge_painter = ui.painter().with_clip_rect(rect);
    draw_orthogonal_edges(&edge_painter, &layouts, &rect_lookup, state, hovered_post.as_ref());

    // Render nodes
    // IMPORTANT: Clip all node rendering to the viewport to prevent overlap with header
    let original_clip = ui.clip_rect();
    ui.set_clip_rect(rect.intersect(original_clip));
    
    let api_base = app.api.base_url().to_string();
    for layout in layouts {
        if !rect.intersects(layout.rect) {
            continue;
        }
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

        let is_secondary = state.secondary_selected_post.as_ref() == Some(&layout.post.id);

        render_node(app, ui, state, &layout, &api_base, rect, state.graph_zoom, &children, is_neighbor, is_secondary);
    }
    
    // Restore clip rect
    ui.set_clip_rect(original_clip);
}

/// Draw orthogonal (Manhattan-style) edges between posts
struct EdgeToDraw {
    start_y: f32,
    end_y: f32,
    parent_rect: egui::Rect,
    child_rect: egui::Rect,
    color: Color32,
    width: f32,
    child_id: String,
    parent_id: String,
}

fn draw_orthogonal_edges(
    painter: &egui::Painter,
    layouts: &[NodeLayoutData],
    rect_lookup: &HashMap<String, egui::Rect>,
    state: &ThreadState,
    hovered_post: Option<&String>,
) {
    let reply_targets = &state.reply_to;

    // Collect edges to draw
    let mut edges = Vec::new();

    for layout in layouts {
        for parent_id in &layout.post.parent_post_ids {
            if let Some(parent_rect) = rect_lookup.get(parent_id) {
                let is_reply_target = reply_targets.contains(&layout.post.id);
                let is_selected = state.selected_post.as_ref() == Some(&layout.post.id);
                let is_parent_selected = state.selected_post.as_ref() == Some(parent_id);
                let is_hovered = hovered_post == Some(&layout.post.id) || hovered_post == Some(parent_id);
                
                let color = if is_reply_target {
                    Color32::from_rgb(255, 190, 92)
                } else if is_selected || is_parent_selected {
                    Color32::from_rgb(250, 208, 108)
                } else if is_hovered {
                    Color32::from_rgb(120, 140, 200)
                } else {
                    Color32::from_rgb(60, 65, 80)
                };
                
                let width = if is_reply_target || is_selected || is_parent_selected { 2.5 } else if is_hovered { 2.0 } else { 1.0 };

                edges.push(EdgeToDraw {
                    start_y: parent_rect.bottom(),
                    end_y: layout.rect.top(),
                    parent_rect: *parent_rect,
                    child_rect: layout.rect,
                    color,
                    width,
                    child_id: layout.post.id.clone(),
                    parent_id: parent_id.clone(),
                });
            }
        }
    }

    // Calculate Port Offsets
    // 1. Group edges by parent (outgoing) and child (incoming)
    let mut outgoing: HashMap<String, Vec<usize>> = HashMap::new();
    let mut incoming: HashMap<String, Vec<usize>> = HashMap::new();

    for (i, edge) in edges.iter().enumerate() {
        outgoing.entry(edge.parent_id.clone()).or_default().push(i);
        incoming.entry(edge.child_id.clone()).or_default().push(i);
    }

    // 2. Sort edges to minimize crossing
    for (_, indices) in outgoing.iter_mut() {
        indices.sort_by(|&a, &b| {
            edges[a].child_rect.center().x.partial_cmp(&edges[b].child_rect.center().x).unwrap_or(std::cmp::Ordering::Equal)
        });
    }
    for (_, indices) in incoming.iter_mut() {
        indices.sort_by(|&a, &b| {
            edges[a].parent_rect.center().x.partial_cmp(&edges[b].parent_rect.center().x).unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    // 3. Calculate offsets for each edge index
    let mut start_offsets: HashMap<usize, f32> = HashMap::new();
    let mut end_offsets: HashMap<usize, f32> = HashMap::new();
    let port_spacing = 10.0 * state.graph_zoom;

    for indices in outgoing.values() {
        let count = indices.len();
        let total_width = (count as f32 - 1.0) * port_spacing;
        let start_x = -total_width / 2.0;
        for (i, &edge_idx) in indices.iter().enumerate() {
            start_offsets.insert(edge_idx, start_x + (i as f32) * port_spacing);
        }
    }
    for indices in incoming.values() {
        let count = indices.len();
        let total_width = (count as f32 - 1.0) * port_spacing;
        let start_x = -total_width / 2.0;
        for (i, &edge_idx) in indices.iter().enumerate() {
            end_offsets.insert(edge_idx, start_x + (i as f32) * port_spacing);
        }
    }

    // Group edges by their vertical span (start_y, end_y) for Lane Logic
    let mut groups: HashMap<(i64, i64), Vec<usize>> = HashMap::new();
    for (i, edge) in edges.iter().enumerate() {
        let key = ((edge.start_y) as i64, (edge.end_y) as i64);
        groups.entry(key).or_default().push(i);
    }

    for (_, indices) in groups.iter_mut() {
        // Sort by child X to keep lines orderly
        indices.sort_by(|&a, &b| edges[a].child_rect.center().x.partial_cmp(&edges[b].child_rect.center().x).unwrap_or(std::cmp::Ordering::Equal));
        
        let count = indices.len();
        if count == 0 { continue; }
        
        let first_edge = &edges[indices[0]];
        let start_y = first_edge.start_y;
        let end_y = first_edge.end_y;
        let span = end_y - start_y;
        let center_y = start_y + span * 0.5;
        
        // Distribute lanes
        let spacing = 12.0 * state.graph_zoom;
        let total_height = (count as f32 - 1.0) * spacing;
        let start_offset = -total_height / 2.0;
        
        for (i, &edge_idx) in indices.iter().enumerate() {
            let edge = &edges[edge_idx];
            let offset = start_offset + (i as f32) * spacing;
            let mid_y = (center_y + offset).min(end_y - 20.0 * state.graph_zoom).max(start_y + 20.0 * state.graph_zoom);
            
            // Apply Port Offsets
            let start_x_off = *start_offsets.get(&edge_idx).unwrap_or(&0.0);
            let end_x_off = *end_offsets.get(&edge_idx).unwrap_or(&0.0);

            let start = edge.parent_rect.center_bottom() + egui::vec2(start_x_off, 0.0);
            let end = edge.child_rect.center_top() + egui::vec2(end_x_off, 0.0);
            
            let p1 = egui::pos2(start.x, mid_y);
            let p2 = egui::pos2(end.x, mid_y);
            
            let stroke = egui::Stroke::new(edge.width, edge.color);
            
            // Draw 3 segments
            painter.line_segment([start, p1], stroke);
            painter.line_segment([p1, p2], stroke);
            painter.line_segment([p2, end], stroke);
            
            // Arrowhead at end
            let arrow_size = 4.0 * state.graph_zoom;
            painter.line_segment(
                [end, end + egui::vec2(-arrow_size, -arrow_size)],
                stroke,
            );
            painter.line_segment(
                [end, end + egui::vec2(arrow_size, -arrow_size)],
                stroke,
            );
        }
    }
}

fn draw_time_axis(
    painter: &egui::Painter,
    posts: &[PostView],
    state: &ThreadState,
    rect: egui::Rect,
) {
    if posts.is_empty() { return; }
    
    // Find min/max time
    let times: Vec<i64> = posts.iter()
        .filter_map(|p| DateTime::parse_from_rfc3339(&p.created_at).ok().map(|dt| dt.timestamp()))
        .collect();
        
    if times.is_empty() { return; }
    
    let min_time = *times.iter().min().unwrap();
    let max_time = *times.iter().max().unwrap();
    let duration = max_time - min_time;
    
    if duration == 0 { return; }
    
    // Draw vertical line
    let line_x = rect.left() + 60.0;
    painter.line_segment(
        [egui::pos2(line_x, rect.top() + 20.0), egui::pos2(line_x, rect.bottom() - 20.0)],
        egui::Stroke::new(1.0, Color32::from_rgb(60, 60, 70))
    );
    
    // Draw ticks?
    // This is tricky because the layout is not linear in time, it's binned.
    // The Y position depends on the bin index and content height.
    // So we can't easily map time to Y without knowing the bin layout.
    // But we have the bin layout in `state.chronological_nodes`.
    // We can iterate through nodes and draw time markers for the first node in each bin?
    
    // For now, just a simple line to indicate time flow
    painter.text(
        egui::pos2(line_x - 10.0, rect.top() + 20.0),
        egui::Align2::RIGHT_TOP,
        "Oldest",
        egui::FontId::proportional(10.0),
        Color32::GRAY,
    );
    
    painter.text(
        egui::pos2(line_x - 10.0, rect.bottom() - 20.0),
        egui::Align2::RIGHT_BOTTOM,
        "Newest",
        egui::FontId::proportional(10.0),
        Color32::GRAY,
    );
}
