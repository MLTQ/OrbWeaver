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
    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };

    // Build layout if not already done or if we just switched to this mode
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

    // 1. Render Canvas
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
        state.graph_zoom = (state.graph_zoom * total_zoom_factor).clamp(0.2, 4.0);
        
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

    // Handle panning with right-click drag
    if response.dragged_by(egui::PointerButton::Secondary) {
        state.graph_offset += response.drag_delta();
    }

    // Pre-calculate children for all posts
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for post in &posts {
        for parent_id in &post.parent_post_ids {
            children_map.entry(parent_id.clone()).or_default().push(post.id.clone());
        }
    }

    // Dynamic Layout Calculation
    // We recalculate positions every frame to ensure no overlaps regardless of content size
    let mut layouts = Vec::new();
    
    // Parse timestamps and sort posts by time (oldest first)
    let mut timestamped_posts: Vec<(DateTime<Utc>, &PostView)> = posts
        .iter()
        .filter_map(|p| match DateTime::parse_from_rfc3339(&p.created_at) {
            Ok(dt) => Some((dt.with_timezone(&Utc), p)),
            Err(_) => None,
        })
        .collect();
    timestamped_posts.sort_by_key(|(dt, _)| *dt);

    let bins = create_time_bins(&timestamped_posts, state.time_bin_seconds);
    
    let mut current_y = TOP_MARGIN;
    
    for bin in bins {
        let mut max_height_in_bin: f32 = 0.0;
        
        for (idx, post_id) in bin.post_ids.iter().enumerate() {
            if let Some(post) = posts.iter().find(|p| &p.id == post_id) {
                let attachments = state.attachments.get(&post.id).cloned();
                let has_preview = attachments
                    .as_ref()
                    .map(|files| files.iter().any(is_image))
                    .unwrap_or(false);
                let has_children = children_map.contains_key(&post.id);

                // Calculate unzoomed height
                let unzoomed_height = estimate_node_height(ui, post, has_preview, has_children, 1.0);
                max_height_in_bin = max_height_in_bin.max(unzoomed_height);

                let x = LEFT_MARGIN + (idx as f32) * (CARD_WIDTH + CARD_HORIZONTAL_SPACING);
                let y = current_y;

                // Update stored node position (for reference/persistence if needed)
                state.graph_nodes.insert(
                    post.id.clone(),
                    GraphNode {
                        pos: egui::pos2(x, y),
                        vel: egui::vec2(0.0, 0.0),
                        size: egui::vec2(CARD_WIDTH, unzoomed_height),
                        dragging: false,
                        pinned: false,
                    },
                );

                // Create layout data
                let top_left = egui::pos2(
                    rect.left() + state.graph_offset.x + x * state.graph_zoom,
                    rect.top() + state.graph_offset.y + y * state.graph_zoom,
                );
                let scaled_size = egui::vec2(CARD_WIDTH, unzoomed_height) * state.graph_zoom;
                let rect_node = egui::Rect::from_min_size(top_left, scaled_size);

                layouts.push(NodeLayoutData {
                    post: post.clone(),
                    rect: rect_node,
                    attachments,
                });
            }
        }
        
        current_y += max_height_in_bin + MIN_BIN_VERTICAL_SPACING;
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
        let children = children_map.get(&layout.post.id).cloned().unwrap_or_default();
        render_node(app, ui, state, layout, &api_base, rect, state.graph_zoom, children);
    }
    
    // Restore clip rect
    ui.set_clip_rect(original_clip);

    // Render Controls in the reserved bottom area
    ui.allocate_ui_at_rect(control_rect, |ui| {
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
}

fn render_node(
    app: &mut GraphchanApp,
    ui: &mut egui::Ui,
    state: &mut ThreadState,
    layout: NodeLayoutData,
    api_base: &str,
    viewport: egui::Rect,
    zoom: f32,
    children: Vec<String>,
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

    let stroke_width = (if hovered { 2.5 } else { 1.5 }) * zoom;
    let stroke_color = if reply_target {
        Color32::from_rgb(255, 190, 92)
    } else if selected {
        Color32::from_rgb(250, 208, 108)
    } else if hovered {
        Color32::from_rgb(120, 140, 200)  // Bright blue on hover
    } else {
        Color32::from_rgb(80, 90, 130)
    };

    // Paint the background and stroke explicitly to match rect_node
    // This ensures the visual box matches the edge connection points exactly,
    // even if the content inside is smaller than the estimated height.
    let rounding = egui::Rounding::same(10.0 * zoom);
    ui.painter().rect(
        rect_node,
        rounding,
        fill_color,
        egui::Stroke::new(stroke_width, stroke_color),
    );

    let response = ui.allocate_ui_at_rect(rect_node, |ui| {
        // Use a transparent frame just for padding/margins
        egui::Frame::none()
            .inner_margin(Margin::same(10.0 * zoom))
            .show(ui, |ui| {
                // Clip to the intersection of the node rect and the viewport (CentralPanel)
                // This prevents drawing over the TopBottomPanel
                ui.set_clip_rect(rect_node.intersect(viewport));
                
                ui.vertical(|ui| {
                    // Use bottom_up layout to anchor replies to the bottom
                    ui.with_layout(egui::Layout::bottom_up(egui::Align::LEFT), |ui| {
                        if !children.is_empty() {
                            // Padding from bottom edge
                            ui.add_space(4.0 * zoom);
                            
                            ui.horizontal_wrapped(|ui| {
                                // Don't set max_width - let frame's inner_margin handle spacing
                                ui.label(RichText::new("↪ Replies:").size(11.0 * zoom).color(Color32::GRAY));
                                for child_id in children {
                                    render_post_link(ui, state, &child_id, zoom, viewport);
                                }
                            });
                            
                            // Spacing between content and replies
                            ui.add_space(4.0 * zoom);
                        }

                        // Render the rest of the content top-down in the remaining space
                        ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                            render_node_header(ui, state, &layout.post, zoom);

                            // Show incoming edges (posts this replies to)
                            if !layout.post.parent_post_ids.is_empty() {
                                ui.add_space(4.0 * zoom);
                                ui.horizontal_wrapped(|ui| {
                                    ui.label(RichText::new("↩ Replying to:").size(11.0 * zoom).color(Color32::GRAY));
                                    for parent in &layout.post.parent_post_ids {
                                        render_post_link(ui, state, parent, zoom, viewport);
                                    }
                                });
                            }

                            ui.add_space(6.0 * zoom);
                            ui.label(
                                egui::RichText::new(&layout.post.body)
                                    .size(13.0 * zoom)
                                    .color(Color32::from_rgb(220, 220, 230)),
                            );

                            render_node_attachments(app, ui, layout.attachments.as_ref(), api_base, zoom);

                            ui.add_space(6.0 * zoom);
                            render_node_actions(ui, state, &layout.post, zoom);
                        });
                    });
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

fn render_post_link(
    ui: &mut egui::Ui,
    state: &mut ThreadState,
    target_id: &str,
    zoom: f32,
    viewport: egui::Rect,
) {
    let short_id = if target_id.len() > 8 { &target_id[..8] } else { target_id };
    let link = ui.link(RichText::new(format!(">>{}", short_id)).size(11.0 * zoom));
    
    if link.clicked() {
        state.selected_post = Some(target_id.to_string());
        
        // Center view on target
        if let Some(node) = state.graph_nodes.get(target_id) {
            let center = viewport.center();
            let target_x = node.pos.x * zoom;
            let target_y = node.pos.y * zoom;
            
            // Center on the node (approximating size as half card width)
            let half_width = (CARD_WIDTH * zoom) / 2.0;
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

fn render_node_header(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView, zoom: f32) {
    ui.horizontal(|ui| {
        if ui
            .button(RichText::new(format!("#{}", post.id)).monospace().size(11.0 * zoom))
            .clicked()
        {
            state.selected_post = Some(post.id.clone());
        }
        let author = post.author_peer_id.as_deref().unwrap_or("Anon");
        ui.label(RichText::new(author).strong().size(12.0 * zoom));
        
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                RichText::new(format_timestamp(&post.created_at))
                    .color(Color32::from_rgb(200, 200, 210))
                    .size(10.0 * zoom),
            );
        });
    });
}

fn render_node_actions(ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView, zoom: f32) {
    ui.horizontal(|ui| {
        // Scale buttons by scaling their text
        if ui.button(RichText::new("↩ Reply").size(13.0 * zoom)).clicked() {
            GraphchanApp::set_reply_target(state, &post.id);
        }
        if ui.button(RichText::new("❝ Quote").size(13.0 * zoom)).clicked() {
            GraphchanApp::quote_post(state, &post.id);
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
    use super::graph::{image_preview, ImagePreview};

    let files = match attachments {
        Some(list) => list,
        None => return,
    };

    if let Some(file) = files.iter().find(|f| is_image(f)) {
        match image_preview(app, ui, file, api_base) {
            ImagePreview::Ready(tex) => {
                ui.add_space(6.0 * zoom);
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
                ui.add_space(6.0 * zoom);
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(16.0 * zoom));
                    ui.label(RichText::new("Fetching image…").size(12.0 * zoom));
                });
            }
            ImagePreview::Error(err) => {
                ui.add_space(6.0 * zoom);
                ui.colored_label(Color32::LIGHT_RED, RichText::new(format!("Image failed: {err}")).size(12.0 * zoom));
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

fn estimate_node_height(ui: &egui::Ui, post: &PostView, has_preview: bool, has_children: bool, zoom: f32) -> f32 {
    use eframe::egui::FontId;

    let text_width = (CARD_WIDTH - 20.0) * zoom;
    let text_height = ui.fonts(|fonts| {
        let body = post.body.clone();
        let galley = fonts.layout(body, FontId::proportional(13.0 * zoom), Color32::WHITE, text_width);
        galley.size().y
    });

    // Base padding (header + footer spacing)
    let mut height = (50.0 * zoom) + text_height;
    
    // Header height approx
    height += 25.0 * zoom;

    if !post.parent_post_ids.is_empty() {
        height += 20.0 * zoom; // "Replying to:" line
    }
    
    if has_preview {
        height += 126.0 * zoom; // Image (120) + padding (6)
    }
    
    if has_children {
        height += 20.0 * zoom; // "Replies:" line
    }
    
    height + (20.0 * zoom) // Bottom padding
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
}

fn draw_orthogonal_edges(
    painter: &egui::Painter,
    layouts: &[NodeLayoutData],
    rect_lookup: &HashMap<String, egui::Rect>,
    state: &ThreadState,
    hovered_post: Option<&String>,
) {
    let reply_targets = &state.reply_to;



    let mut edges = Vec::new();

    // 1. Collect all edges
    for layout in layouts {
        for parent_id in &layout.post.parent_post_ids {
            let (parent_rect, child_rect) =
                match (rect_lookup.get(parent_id), rect_lookup.get(&layout.post.id)) {
                    (Some(p), Some(c)) => (*p, *c),
                    _ => continue,
                };

            let is_reply_edge = reply_targets
                .iter()
                .any(|id| id == parent_id || id == &layout.post.id);
            
            let is_hovered = hovered_post.map_or(false, |hovered| {
                hovered == parent_id || hovered == &layout.post.id
            });

            let sel = state.selected_post.as_ref();
            let color = if is_reply_edge {
                Color32::from_rgb(255, 190, 92)
            } else if is_hovered {
                Color32::from_rgb(150, 180, 255)
            } else if sel == Some(&layout.post.id) || sel == Some(parent_id) {
                Color32::from_rgb(110, 190, 255)
            } else {
                Color32::from_rgb(90, 110, 170)
            };

            let width = (if is_reply_edge { 3.4 } else if is_hovered { 3.0 } else { 2.0 }) * state.graph_zoom;

            edges.push(EdgeToDraw {
                start_y: parent_rect.bottom(),
                end_y: child_rect.top(),
                parent_rect,
                child_rect,
                color,
                width,
                child_id: layout.post.id.clone(), // Store for sorting
            });
        }
    }

    // 2. Sort edges by start_y (top to bottom), then by child_id for stability
    edges.sort_by(|a, b| {
        a.start_y
            .partial_cmp(&b.start_y)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.child_id.cmp(&b.child_id))
    });

    // 3. Assign lanes (greedy interval coloring)
    // lanes[i] stores the y-coordinate where lane i becomes free
    let mut lanes: Vec<f32> = Vec::new();
    let lane_spacing = 12.0 * state.graph_zoom;
    let base_offset = 20.0 * state.graph_zoom;

    for edge in edges {
        // Find first free lane
        let mut lane_index = 0;
        let mut found = false;
        for (i, free_y) in lanes.iter_mut().enumerate() {
            if *free_y < edge.start_y {
                *free_y = edge.end_y;
                lane_index = i;
                found = true;
                break;
            }
        }
        if !found {
            lane_index = lanes.len();
            lanes.push(edge.end_y);
        }

        // 4. Draw edge using lane
        // Route to the LEFT of the parent/child
        // X coordinate for this lane
        let lane_x = edge.parent_rect.left() - base_offset - (lane_index as f32 * lane_spacing);
        
        let start = anchor_bottom(&edge.parent_rect);
        let end = anchor_top(&edge.child_rect);
        
        let corner_radius = 8.0 * state.graph_zoom;

        // Path points
        let p1 = egui::pos2(start.x, start.y + corner_radius);
        let p2 = egui::pos2(lane_x, start.y + corner_radius);
        let p3 = egui::pos2(lane_x, end.y - corner_radius);
        let p4 = egui::pos2(end.x, end.y - corner_radius);

        // Draw path
        let points = vec![start, p1, p2, p3, p4, end];
        
        // Simplify if points are close (e.g. straight down)
        // But here we are forcing a lane detour, so we always have these points.
        // Unless lane_x is actually between parent and child? No, we forced it left.
        
        painter.add(egui::Shape::line(
            points,
            egui::Stroke::new(edge.width, edge.color),
        ));
        
        draw_arrow(painter, p4, end, edge.color);
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
