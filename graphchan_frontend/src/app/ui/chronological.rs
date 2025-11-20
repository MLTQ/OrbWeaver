use std::collections::HashMap;

use chrono::{DateTime, Timelike, Utc};
use eframe::egui::{self, Color32, Margin, Pos2, RichText};
use log;

use crate::models::{FileResponse, PostView};

use super::super::state::{GraphNode, ThreadState};
use super::super::{format_timestamp, GraphchanApp};

/// Configuration for chronological layout

const CARD_WIDTH: f32 = 720.0; // Match 4chan width for familiarity
const CARD_HORIZONTAL_SPACING: f32 = 50.0; // More space between cards
const MIN_BIN_VERTICAL_SPACING: f32 = 50.0; // Minimum spacing between bins
const LEFT_MARGIN: f32 = 120.0; // Leave room for time axis
const TOP_MARGIN: f32 = 50.0;
const EDGE_CLEARANCE: f32 = 15.0; // Space to route around cards

struct ChronoBin {
    timestamp: DateTime<Utc>,
    post_ids: Vec<String>,
}

struct NodeLayoutData {
    post: PostView,
    rect: egui::Rect,
    attachments: Option<Vec<FileResponse>>,
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
    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };

    // Build layout if not already done or if we just switched to this mode
    if state.graph_nodes.is_empty() {
        state.graph_nodes = build_chronological_layout(&posts, state.time_bin_seconds);
    }

    let available = ui.available_size();
    let (rect, response) = ui.allocate_at_least(available, egui::Sense::click_and_drag());
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

    // Handle panning with right-click drag
    if response.dragged_by(egui::PointerButton::Secondary) {
        state.graph_offset += response.drag_delta();
    }

    // Build layout data for rendering
    let mut layouts = Vec::new();
    let api_base = app.api.base_url().to_string();

    for post in posts.iter() {
        if let Some(node) = state.graph_nodes.get_mut(&post.id) {
            let attachments = state.attachments.get(&post.id).cloned();
            let has_preview = attachments
                .as_ref()
                .map(|files| files.iter().any(is_image))
                .unwrap_or(false);

            let card_height = estimate_node_height(ui, post, has_preview);
            node.size = egui::vec2(CARD_WIDTH, card_height);

            // Apply zoom and offset to position
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

    // Draw orthogonal edges
    let edge_painter = ui.painter().with_clip_rect(rect);
    draw_orthogonal_edges(&edge_painter, &layouts, &rect_lookup, state, hovered_post.as_ref());

    // Render nodes
    for layout in layouts {
        render_node(app, ui, state, layout, &api_base, rect);
    }

    ui.separator();
    ui.horizontal(|ui| {
        ui.label(format!(
            "Chronological View | Nodes: {}",
            state.graph_nodes.len()
        ));
        
        ui.separator();
        
        // Time bin size slider
        ui.label("Time Bin:");
        let mut bin_minutes = (state.time_bin_seconds / 60) as f32;
        let mut rebuild_layout = false;
        if ui.add(egui::Slider::new(&mut bin_minutes, 1.0..=120.0)
            .suffix(" min")
            .logarithmic(true))
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
            state.graph_nodes = build_chronological_layout(&posts, state.time_bin_seconds);
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
}

fn render_node(
    app: &mut GraphchanApp,
    ui: &mut egui::Ui,
    state: &mut ThreadState,
    layout: NodeLayoutData,
    api_base: &str,
    _viewport: egui::Rect,
) {
    let rect_node = layout.rect;
    let selected = state.selected_post.as_ref() == Some(&layout.post.id);
    let reply_target = state.reply_to.iter().any(|id| id == &layout.post.id);
    
    // Check if mouse is hovering OR if this post is locked as hovered
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
        Color32::from_rgb(42, 45, 60)  // Lighter on hover
    } else {
        Color32::from_rgb(30, 30, 38)
    };

    let stroke_width = if hovered { 2.5 } else { 1.5 };
    let stroke_color = if reply_target {
        Color32::from_rgb(255, 190, 92)
    } else if selected {
        Color32::from_rgb(250, 208, 108)
    } else if hovered {
        Color32::from_rgb(120, 140, 200)  // Bright blue on hover
    } else {
        Color32::from_rgb(80, 90, 130)
    };

    let response = ui.allocate_ui_at_rect(rect_node, |ui| {
        egui::Frame::none()
            .fill(fill_color)
            .stroke(egui::Stroke::new(stroke_width, stroke_color))
            .rounding(egui::Rounding::same(10.0))
            .inner_margin(Margin::same(10.0))
            .show(ui, |ui| {
                ui.set_clip_rect(rect_node);
                ui.vertical(|ui| {
                    // Don't set max_width - let frame's inner_margin handle spacing
                    
                    render_node_header(ui, state, &layout.post);

                    // Show incoming edges (posts this replies to)
                    if !layout.post.parent_post_ids.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.label(RichText::new("↩ Replying to:").size(11.0).color(Color32::GRAY));
                            for parent in &layout.post.parent_post_ids {
                                if ui.link(RichText::new(format!("#{}", parent)).size(11.0)).clicked() {
                                    state.selected_post = Some(parent.clone());
                                }
                            }
                        });
                    }

                    ui.add_space(6.0);
                    ui.label(
                        egui::RichText::new(&layout.post.body)
                            .size(13.0)
                            .color(Color32::from_rgb(220, 220, 230)),
                    );

                    render_node_attachments(app, ui, layout.attachments.as_ref(), api_base);

                    ui.add_space(6.0);
                    render_node_actions(ui, state, &layout.post);
                    
                    // Show outgoing edges (posts that reply to this one)
                    // We need to find these by scanning all posts
                    let children: Vec<String> = state.details
                        .as_ref()
                        .map(|d| {
                            d.posts.iter()
                                .filter(|p| p.parent_post_ids.contains(&layout.post.id))
                                .map(|p| p.id.clone())
                                .collect()
                        })
                        .unwrap_or_default();
                    
                    if !children.is_empty() {
                        ui.add_space(4.0);
                        ui.horizontal_wrapped(|ui| {
                            // Don't set max_width - let frame's inner_margin handle spacing
                            ui.label(RichText::new("↪ Replies:").size(11.0).color(Color32::GRAY));
                            for child_id in children {
                                if ui.link(RichText::new(format!("#{}", child_id)).size(11.0)).clicked() {
                                    state.selected_post = Some(child_id);
                                }
                            }
                        });
                    }
                });
            })
    }).response;

    // Handle click to select and toggle locked hover
    if response.clicked() {
        state.selected_post = Some(layout.post.id.clone());
        // Toggle locked hover - if already locked on this post, unlock it
        if state.locked_hover_post.as_ref() == Some(&layout.post.id) {
            state.locked_hover_post = None;
        } else {
            state.locked_hover_post = Some(layout.post.id.clone());
        }
    }
}

fn render_node_header(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView) {
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new(format!("#{}", post.id)).monospace().size(11.0))
            .clicked()
        {
            state.selected_post = Some(post.id.clone());
        }
        let author = post.author_peer_id.as_deref().unwrap_or("Anon");
        ui.label(RichText::new(author).strong().size(12.0));
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format_timestamp(&post.created_at))
                    .color(Color32::from_rgb(200, 200, 210))
                    .size(10.0),
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
    use super::graph::{image_preview, ImagePreview};

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

fn is_image(file: &FileResponse) -> bool {
    file.present
        && file
            .mime
            .as_deref()
            .map(|m| m.starts_with("image/"))
            .unwrap_or(false)
}

fn estimate_node_height(ui: &egui::Ui, post: &PostView, has_preview: bool) -> f32 {
    use eframe::egui::FontId;

    let text_width = CARD_WIDTH - 20.0;
    let text_height = ui.fonts(|fonts| {
        let body = post.body.clone();
        let galley = fonts.layout(body, FontId::proportional(13.0), Color32::WHITE, text_width);
        galley.size().y
    });

    let mut height = 70.0 + text_height;
    if !post.parent_post_ids.is_empty() {
        height += 28.0;
    }
    if has_preview {
        height += 120.0;
    }
    height + 36.0
}

/// Draw orthogonal (Manhattan-style) edges between posts
fn draw_orthogonal_edges(
    painter: &egui::Painter,
    layouts: &[NodeLayoutData],
    rect_lookup: &HashMap<String, egui::Rect>,
    state: &ThreadState,
    hovered_post: Option<&String>,
) {
    let reply_targets = &state.reply_to;

    for layout in layouts {
        for parent_id in &layout.post.parent_post_ids {
            let (parent_rect, child_rect) =
                match (rect_lookup.get(parent_id), rect_lookup.get(&layout.post.id)) {
                    (Some(p), Some(c)) => (p, c),
                    _ => continue,
                };

            let is_reply_edge = reply_targets
                .iter()
                .any(|id| id == parent_id || id == &layout.post.id);
            
            // Highlight if hovering over either end of this edge
            let is_hovered = hovered_post.map_or(false, |hovered| {
                hovered == parent_id || hovered == &layout.post.id
            });

            let sel = state.selected_post.as_ref();
            let color = if is_reply_edge {
                Color32::from_rgb(255, 190, 92)
            } else if is_hovered {
                Color32::from_rgb(150, 180, 255)  // Bright blue on hover
            } else if sel == Some(&layout.post.id) || sel == Some(parent_id) {
                Color32::from_rgb(110, 190, 255)
            } else {
                Color32::from_rgb(90, 110, 170)
            };

            let stroke_width = if is_reply_edge { 3.4 } else if is_hovered { 3.0 } else { 2.0 };

            draw_manhattan_path(painter, parent_rect, child_rect, color, stroke_width);
        }
    }
}

/// Draw a Manhattan/orthogonal path between two rects
fn draw_manhattan_path(
    painter: &egui::Painter,
    parent_rect: &egui::Rect,
    child_rect: &egui::Rect,
    color: Color32,
    stroke_width: f32,
) {
    let start = anchor_bottom(parent_rect);
    let end = anchor_top(child_rect);

    // Simple case: child is directly below parent
    if child_rect.top() > parent_rect.bottom() + 10.0 {
        // Check if horizontally aligned enough for straight line
        let horizontal_distance = (child_rect.center().x - parent_rect.center().x).abs();

        if horizontal_distance < 50.0 {
            // Straight down
            let points = vec![start, end];
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(stroke_width, color),
            ));
            draw_arrow(painter, start, end, color);
        } else {
            // S-curve down
            let mid_y = (start.y + end.y) / 2.0;
            let waypoint1 = Pos2::new(start.x, mid_y);
            let waypoint2 = Pos2::new(end.x, mid_y);

            let points = vec![start, waypoint1, waypoint2, end];
            painter.add(egui::Shape::line(
                points,
                egui::Stroke::new(stroke_width, color),
            ));
            draw_arrow(painter, waypoint2, end, color);
        }
    } else {
        // Complex case: child is above or at same level as parent - route around
        let clearance_up = parent_rect.top() - EDGE_CLEARANCE;
        let clearance_right = parent_rect.right() + EDGE_CLEARANCE;

        let waypoint1 = Pos2::new(start.x, clearance_up);
        let waypoint2 = Pos2::new(clearance_right, clearance_up);
        let waypoint3 = Pos2::new(clearance_right, end.y - EDGE_CLEARANCE);
        let waypoint4 = Pos2::new(end.x, end.y - EDGE_CLEARANCE);

        let points = vec![start, waypoint1, waypoint2, waypoint3, waypoint4, end];
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(stroke_width, color),
        ));
        draw_arrow(painter, waypoint4, end, color);
    }
}

fn anchor_top(rect: &egui::Rect) -> Pos2 {
    Pos2::new(rect.center().x, rect.top() - 6.0)
}

fn anchor_bottom(rect: &egui::Rect) -> Pos2 {
    Pos2::new(rect.center().x, rect.bottom() + 6.0)
}

fn draw_arrow(painter: &egui::Painter, from: Pos2, to: Pos2, color: Color32) {
    let dir = (to - from).normalized();
    let normal = egui::Vec2::new(-dir.y, dir.x);
    let arrow_size = 8.0;

    let arrow_tip = to;
    let arrow_left = Pos2::new(
        to.x - dir.x * arrow_size + normal.x * arrow_size * 0.5,
        to.y - dir.y * arrow_size + normal.y * arrow_size * 0.5,
    );
    let arrow_right = Pos2::new(
        to.x - dir.x * arrow_size - normal.x * arrow_size * 0.5,
        to.y - dir.y * arrow_size - normal.y * arrow_size * 0.5,
    );

    painter.add(egui::Shape::convex_polygon(
        vec![arrow_tip, arrow_left, arrow_right],
        color,
        egui::Stroke::NONE,
    ));
}

fn draw_time_axis(
    painter: &egui::Painter,
    posts: &[PostView],
    state: &ThreadState,
    viewport: egui::Rect,
) {
    // Draw time labels on the left side for each post's y position
    for (post_id, node) in &state.graph_nodes {
        // Find the post to get its timestamp
        let post = posts.iter().find(|p| &p.id == post_id);
        if let Some(post) = post {
            if let Ok(dt) = DateTime::parse_from_rfc3339(&post.created_at) {
                let utc_dt = dt.with_timezone(&Utc);
                let time_str = utc_dt.format("%H:%M:%S").to_string();
                
                // Calculate screen position with zoom and offset
                let screen_y = viewport.top() + state.graph_offset.y + node.pos.y * state.graph_zoom;
                let screen_x = viewport.left() + 10.0; // Fixed position on left
                
                // Only draw if on screen
                if screen_y >= viewport.top() && screen_y <= viewport.bottom() {
                    painter.text(
                        Pos2::new(screen_x, screen_y),
                        egui::Align2::LEFT_CENTER,
                        time_str,
                        egui::FontId::monospace(10.0),
                        Color32::from_rgb(150, 150, 160),
                    );
                }
            }
        }
    }
}
