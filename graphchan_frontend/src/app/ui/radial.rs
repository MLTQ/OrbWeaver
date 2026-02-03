use std::collections::HashMap;
use std::f32::consts::PI;

use eframe::egui::{self, Color32, Pos2, Vec2};

use crate::models::PostView;

use super::super::state::{RadialNode, ThreadState};
use super::super::GraphchanApp;
use super::node::{estimate_node_size, is_image, render_node, NodeLayoutData};

const RING_SPACING: f32 = 400.0; // Distance between rings
const CENTER_RADIUS: f32 = 200.0; // Radius of the center area (for OP)
const ROTATION_SPEED: f32 = 0.15; // How fast the table rotates (lerp factor)

/// Compute ring depths for all posts using longest-path algorithm.
/// ring(OP) = 0
/// ring(post) = max(ring(parent) for all parents) + 1
fn compute_ring_depths(posts: &[PostView], op_id: &str) -> HashMap<String, usize> {
    let mut depths: HashMap<String, usize> = HashMap::new();

    // Build a map from post ID to post for quick lookup
    let post_map: HashMap<&str, &PostView> = posts.iter()
        .map(|p| (p.id.as_str(), p))
        .collect();

    // Recursive function to compute depth with memoization
    fn get_depth(
        post_id: &str,
        op_id: &str,
        post_map: &HashMap<&str, &PostView>,
        depths: &mut HashMap<String, usize>,
    ) -> usize {
        // Check memoization
        if let Some(&d) = depths.get(post_id) {
            return d;
        }

        // OP is always ring 0
        if post_id == op_id {
            depths.insert(post_id.to_string(), 0);
            return 0;
        }

        let post = match post_map.get(post_id) {
            Some(p) => p,
            None => {
                // Unknown post, treat as ring 1
                depths.insert(post_id.to_string(), 1);
                return 1;
            }
        };

        // If no parents or only references OP, it's ring 1
        if post.parent_post_ids.is_empty() {
            depths.insert(post_id.to_string(), 1);
            return 1;
        }

        // Find max depth of all parents
        let max_parent_depth = post.parent_post_ids.iter()
            .map(|pid| get_depth(pid, op_id, post_map, depths))
            .max()
            .unwrap_or(0);

        let depth = max_parent_depth + 1;
        depths.insert(post_id.to_string(), depth);
        depth
    }

    // Compute depths for all posts
    for post in posts {
        get_depth(&post.id, op_id, &post_map, &mut depths);
    }

    depths
}

/// Build the radial layout - assigns ring and angular position to each post
pub fn build_radial_layout(posts: &[PostView]) -> HashMap<String, RadialNode> {
    let mut nodes = HashMap::new();

    if posts.is_empty() {
        return nodes;
    }

    // Find OP (earliest post)
    let op_id = posts.iter()
        .min_by_key(|p| &p.created_at)
        .map(|p| p.id.clone())
        .unwrap_or_default();

    // Compute ring depths
    let depths = compute_ring_depths(posts, &op_id);

    // Group posts by ring
    let mut rings: HashMap<usize, Vec<&PostView>> = HashMap::new();
    for post in posts {
        let ring = *depths.get(&post.id).unwrap_or(&1);
        rings.entry(ring).or_default().push(post);
    }

    // Sort posts within each ring by creation time for consistent ordering
    for posts_in_ring in rings.values_mut() {
        posts_in_ring.sort_by_key(|p| &p.created_at);
    }

    // Assign angular positions
    for (ring, posts_in_ring) in &rings {
        let count = posts_in_ring.len();

        if *ring == 0 {
            // OP at center
            for post in posts_in_ring {
                nodes.insert(post.id.clone(), RadialNode {
                    ring: 0,
                    angle: 0.0,
                    size: egui::vec2(320.0, 150.0),
                });
            }
        } else {
            // Distribute evenly around the ring
            for (i, post) in posts_in_ring.iter().enumerate() {
                let angle = if count == 1 {
                    0.0 // Single post at top
                } else {
                    // Distribute evenly, starting from top (negative Y)
                    (i as f32 / count as f32) * 2.0 * PI - PI / 2.0
                };

                nodes.insert(post.id.clone(), RadialNode {
                    ring: *ring,
                    angle,
                    size: egui::vec2(320.0, 150.0),
                });
            }
        }
    }

    nodes
}

/// Convert radial coordinates to screen position
fn radial_to_screen(
    ring: usize,
    angle: f32,
    center: Pos2,
    zoom: f32,
    rotation: f32,
) -> Pos2 {
    if ring == 0 {
        return center;
    }

    let radius = (CENTER_RADIUS + (ring as f32) * RING_SPACING) * zoom;
    let adjusted_angle = angle + rotation;

    Pos2::new(
        center.x + radius * adjusted_angle.cos(),
        center.y + radius * adjusted_angle.sin(),
    )
}

/// Get the angle needed to bring a post to the "front" (bottom of screen, facing user)
fn angle_to_front(node: &RadialNode) -> f32 {
    // We want the post at the bottom (PI/2 radians = pointing down)
    // So we need to rotate by (PI/2 - node.angle)
    PI / 2.0 - node.angle
}

/// Main render function for the radial phylogenetic tree view
pub fn render_radial(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ThreadState) {
    let posts = match &state.details {
        Some(d) => d.posts.clone(),
        None => return,
    };

    let thread_id = state.summary.id.clone();

    // Initialize radial layout if empty
    // Only reset rotation when FIRST initializing (empty), not on size mismatch
    let was_empty = state.radial_nodes.is_empty();
    if was_empty || state.radial_nodes.len() != posts.len() {
        state.radial_nodes = build_radial_layout(&posts);
        // Only reset rotation on first initialization, not when posts change
        if was_empty {
            state.radial_rotation = 0.0;
            state.radial_target_rotation = 0.0;
        }
    }

    // Update node sizes
    let mut children_map: HashMap<String, Vec<String>> = HashMap::new();
    for post in &posts {
        for parent_id in &post.parent_post_ids {
            children_map.entry(parent_id.clone()).or_default().push(post.id.clone());
        }
    }

    for post in &posts {
        if let Some(node) = state.radial_nodes.get_mut(&post.id) {
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
            node.size = estimate_node_size(ui, post, has_preview, children_count);
        }
    }

    // Animate rotation towards target
    let rotation_diff = state.radial_target_rotation - state.radial_rotation;
    if rotation_diff.abs() > 0.001 {
        state.radial_rotation += rotation_diff * ROTATION_SPEED;
        ui.ctx().request_repaint();
    }

    // Allocate canvas
    let available = ui.available_size();
    let (rect, response) = ui.allocate_at_least(available, egui::Sense::click_and_drag());

    let canvas = ui.painter().with_clip_rect(rect);

    // Dark background
    canvas.rect_filled(rect, 0.0, Color32::from_rgb(12, 13, 20));

    let center = rect.center() + state.graph_offset;
    let zoom = state.graph_zoom;
    let rotation = state.radial_rotation;

    // Draw dot grid
    let dot_spacing = 50.0 * zoom;
    let dot_color = Color32::from_rgba_premultiplied(70, 72, 95, 60);

    if dot_spacing > 10.0 {
        let mut x = (center.x + state.graph_offset.x) % dot_spacing;
        if x < rect.left() { x += dot_spacing; }
        while x < rect.right() {
            let mut y = (center.y + state.graph_offset.y) % dot_spacing;
            if y < rect.top() { y += dot_spacing; }
            while y < rect.bottom() {
                canvas.circle_filled(egui::pos2(x, y), 1.5, dot_color);
                y += dot_spacing;
            }
            x += dot_spacing;
        }
    }

    // Draw concentric ring guides
    let max_ring = state.radial_nodes.values()
        .map(|n| n.ring)
        .max()
        .unwrap_or(0);

    let ring_color = Color32::from_rgba_premultiplied(60, 65, 90, 40);
    for ring in 1..=max_ring {
        let radius = (CENTER_RADIUS + (ring as f32) * RING_SPACING) * zoom;
        canvas.circle_stroke(center, radius, egui::Stroke::new(1.0, ring_color));
    }

    // Handle zoom
    if let Some(pointer_pos) = ui.ctx().pointer_hover_pos() {
        if rect.contains(pointer_pos) {
            let scroll = ui.input(|i| i.raw_scroll_delta.y);
            if scroll != 0.0 {
                let old_zoom = state.graph_zoom;
                let zoom_factor = (1.0 + scroll * 0.001).clamp(0.5, 3.5);
                state.graph_zoom = (old_zoom * zoom_factor).clamp(0.05, 5.0);

                // Zoom towards mouse
                let pointer_rel = pointer_pos - center;
                let zoom_ratio = state.graph_zoom / old_zoom;
                state.graph_offset += pointer_rel - pointer_rel * zoom_ratio;
            }
        }
    }

    // Handle pan
    if response.dragged_by(egui::PointerButton::Secondary) || response.dragged_by(egui::PointerButton::Middle) {
        state.graph_offset += response.drag_delta();
    }

    // Click to deselect
    if response.clicked() {
        state.selected_post = None;
    }

    // Draw edges
    draw_radial_edges(&canvas, &posts, state, center, zoom, rotation);

    // Collect and sort layouts (selected on top)
    let mut layouts = Vec::new();
    let api_base = app.api.base_url().to_string();

    for post in posts.iter() {
        if let Some(node) = state.radial_nodes.get(&post.id) {
            let attachments = if !post.files.is_empty() {
                Some(post.files.clone())
            } else {
                state.attachments.get(&post.id).cloned()
            };

            let screen_pos = radial_to_screen(node.ring, node.angle, center, zoom, rotation);
            let size = node.size * zoom;

            // Calculate rotation for this node (top faces center)
            let node_rotation = if node.ring == 0 {
                0.0 // OP doesn't rotate
            } else {
                node.angle + rotation + PI / 2.0 // Rotate so top faces center
            };

            let top_left = screen_pos - size / 2.0;
            let rect_node = egui::Rect::from_min_size(top_left, size);

            // Only render if visible
            if rect.intersects(rect_node) {
                layouts.push((
                    NodeLayoutData {
                        post: post.clone(),
                        rect: rect_node,
                        attachments,
                    },
                    node_rotation,
                    screen_pos,
                ));
            }
        }
    }

    // Sort so selected is rendered last (on top)
    layouts.sort_by_key(|(l, _, _)| state.selected_post.as_ref() == Some(&l.post.id));

    // Render nodes
    for (layout, node_rotation, screen_pos) in layouts {
        let children = children_map.get(&layout.post.id).cloned().unwrap_or_default();

        // Handle node interaction
        let drag_id = ui.make_persistent_id(format!("radial_node_{}", layout.post.id));
        let drag_handle = ui.interact(layout.rect, drag_id, egui::Sense::click());

        if drag_handle.clicked() {
            let post_id = layout.post.id.clone();
            state.selected_post = Some(post_id.clone());

            // Calculate rotation to bring this post to the front
            if let Some(node) = state.radial_nodes.get(&post_id) {
                if node.ring > 0 {
                    state.radial_target_rotation = angle_to_front(node);
                }
            }
        }

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

        // Render with rotation transform
        // For now, we render without actual rotation transform (egui doesn't easily support per-widget rotation)
        // The "rotation" effect comes from the positions moving around the circle
        // TODO: Implement actual card rotation using egui's transform or custom rendering

        render_node(
            app,
            ui,
            state,
            &layout,
            &api_base,
            rect,
            zoom,
            &children,
            is_neighbor,
            is_secondary,
        );
    }

    // Controls overlay
    let control_rect = egui::Rect::from_min_size(
        rect.min + egui::vec2(10.0, rect.height() - 40.0),
        egui::vec2(rect.width() - 20.0, 30.0),
    );

    ui.allocate_ui_at_rect(control_rect, |ui| {
        ui.horizontal(|ui| {
            ui.label(format!(
                "Zoom: {:.2} | Rings: {} | Posts: {}",
                state.graph_zoom,
                max_ring,
                state.radial_nodes.len()
            ));

            ui.separator();

            if ui.button("Reset View").clicked() {
                state.radial_nodes = build_radial_layout(&posts);
                state.radial_rotation = 0.0;
                state.radial_target_rotation = 0.0;
                state.graph_offset = egui::vec2(0.0, 0.0);
                state.graph_zoom = 1.0;
            }

            ui.separator();

            if ui.button("⟲ Rotate Left").clicked() {
                state.radial_target_rotation += PI / 4.0;
            }
            if ui.button("⟳ Rotate Right").clicked() {
                state.radial_target_rotation -= PI / 4.0;
            }
        });
    });
}

/// Draw edges connecting posts to their parents
fn draw_radial_edges(
    painter: &egui::Painter,
    posts: &[PostView],
    state: &ThreadState,
    center: Pos2,
    zoom: f32,
    rotation: f32,
) {
    let nodes = &state.radial_nodes;

    for post in posts {
        let child_node = match nodes.get(&post.id) {
            Some(n) => n,
            None => continue,
        };

        let child_pos = radial_to_screen(child_node.ring, child_node.angle, center, zoom, rotation);

        for parent_id in &post.parent_post_ids {
            let parent_node = match nodes.get(parent_id) {
                Some(n) => n,
                None => continue,
            };

            let parent_pos = radial_to_screen(parent_node.ring, parent_node.angle, center, zoom, rotation);

            // Determine edge color
            let is_reply_edge = state.reply_to.iter().any(|id| id == parent_id || id == &post.id);
            let sel = state.selected_post.as_ref();
            let is_selected_edge = sel == Some(&post.id) || sel == Some(parent_id);

            let color = if is_reply_edge {
                Color32::from_rgb(255, 190, 92)
            } else if is_selected_edge {
                Color32::from_rgb(250, 208, 108)
            } else {
                Color32::from_rgb(90, 110, 170)
            };

            let stroke_width = if is_reply_edge || is_selected_edge { 3.4 * zoom } else { 2.0 * zoom };

            // Draw curved edge using quadratic bezier
            // Control point is offset towards the center for a nice curve
            let mid = Pos2::new(
                (parent_pos.x + child_pos.x) / 2.0,
                (parent_pos.y + child_pos.y) / 2.0,
            );

            // Pull control point towards center for curve effect
            let to_center = center - mid;
            let curve_strength = 0.3;
            let control = mid + to_center * curve_strength;

            // Draw as bezier curve
            let points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let t = i as f32 / 20.0;
                    let inv_t = 1.0 - t;
                    Pos2::new(
                        inv_t * inv_t * parent_pos.x + 2.0 * inv_t * t * control.x + t * t * child_pos.x,
                        inv_t * inv_t * parent_pos.y + 2.0 * inv_t * t * control.y + t * t * child_pos.y,
                    )
                })
                .collect();

            painter.add(egui::Shape::line(points.clone(), egui::Stroke::new(stroke_width, color)));

            // Draw arrow at child end
            if let (Some(&second_last), Some(&last)) = (points.get(points.len() - 2), points.last()) {
                draw_arrow(painter, second_last, last, color, zoom);
            }
        }
    }
}

fn draw_arrow(painter: &egui::Painter, from: Pos2, to: Pos2, color: Color32, zoom: f32) {
    let dir = (to - from).normalized();
    let normal = Vec2::new(-dir.y, dir.x);
    let arrow_size = 8.0 * zoom;
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
