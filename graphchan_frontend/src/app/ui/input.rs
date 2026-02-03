use std::f32::consts::PI;
use eframe::egui;
use crate::app::GraphchanApp;
use crate::app::state::{ViewState, ThreadState, ThreadDisplayMode};

pub fn handle_keyboard_input(app: &mut GraphchanApp, ctx: &egui::Context) {
    // Only handle keyboard input when viewing a thread
    if let ViewState::Thread(state) = &mut app.view {
        // Check if a TextEdit (draft window) has focus - if so, disable all keyboard controls
        if is_text_edit_focused(ctx) {
            return;
        }

        // Aggressively surrender any UI focus when Tab is pressed in thread view
        // This prevents egui from cycling through UI elements
        let should_surrender = ctx.input(|i| i.key_pressed(egui::Key::Tab));
        if should_surrender {
            ctx.memory_mut(|mem| {
                if let Some(id) = mem.focused() {
                    mem.surrender_focus(id);
                }
            });
        }

        if let Some(details) = &state.details {
            let posts = &details.posts;
            if posts.is_empty() { return; }

            // Auto-select OP on first relevant keypress if nothing selected
            if state.selected_post.is_none() {
                if should_auto_select_op(ctx) {
                    if let Some(first) = posts.first() {
                        state.selected_post = Some(first.id.clone());
                        state.secondary_selected_post = None;
                        state.parent_cursor_index = 0;
                        state.reply_cursor_index = 0;
                        // Consume the key that triggered selection
                        consume_navigation_keys(ctx);
                        ctx.request_repaint();
                    }
                }
                return;
            }

            // We have a selected post - handle navigation
            let current_id = state.selected_post.as_ref().unwrap().clone();

            // Find current post and its index
            let current_index = posts.iter().position(|p| p.id == current_id);
            let current_post = match current_index.and_then(|i| posts.get(i)) {
                Some(p) => p,
                None => return,
            };

            // Clone parent IDs to avoid holding immutable borrow
            let parent_ids = current_post.parent_post_ids.clone();
            let has_parents = !parent_ids.is_empty();

            // Calculate replies (children) - sorted chronologically, extract IDs
            let mut reply_tuples: Vec<(String, String)> = posts.iter()
                .filter(|p| p.parent_post_ids.contains(&current_id))
                .map(|p| (p.id.clone(), p.created_at.clone()))
                .collect();
            reply_tuples.sort_by(|a, b| a.1.cmp(&b.1)); // Sort by created_at
            let reply_ids: Vec<String> = reply_tuples.into_iter().map(|(id, _)| id).collect();
            let has_replies = !reply_ids.is_empty();

            // Clone posts count for Tab navigation
            let posts_len = posts.len();

            let mut handled = false;
            let mut center_on: Option<String> = None;

            // TAB: Next post chronologically
            if consume_key(ctx, egui::Key::Tab, egui::Modifiers::NONE) {
                if let Some(idx) = current_index {
                    let new_index = (idx + 1) % posts_len;
                    if let Some(next_post) = posts.get(new_index) {
                        let next_id = next_post.id.clone();
                        state.selected_post = Some(next_id.clone());
                        state.secondary_selected_post = None;
                        state.parent_cursor_index = 0;
                        state.reply_cursor_index = 0;
                        center_on = Some(next_id);
                        handled = true;
                    }
                }
            }

            // SHIFT+TAB: Previous post chronologically
            if consume_key(ctx, egui::Key::Tab, egui::Modifiers::SHIFT) {
                if let Some(idx) = current_index {
                    let new_index = if idx == 0 { posts_len - 1 } else { idx - 1 };
                    if let Some(prev_post) = posts.get(new_index) {
                        let prev_id = prev_post.id.clone();
                        state.selected_post = Some(prev_id.clone());
                        state.secondary_selected_post = None;
                        state.parent_cursor_index = 0;
                        state.reply_cursor_index = 0;
                        center_on = Some(prev_id);
                        handled = true;
                    }
                }
            }

            // SPACE: Smart selection based on last navigation mode
            // In Radial mode: re-center view on currently selected post
            if consume_key(ctx, egui::Key::Space, egui::Modifiers::NONE) {
                if state.display_mode == ThreadDisplayMode::Radial {
                    // In Radial mode, Space re-centers viewport on the currently selected post
                    center_on = Some(current_id.clone());
                    handled = true;
                    ctx.request_repaint();
                } else {
                    // Other modes: smart selection based on last navigation mode
                    use crate::app::state::NavigationMode;

                    let target_post = match state.last_navigation_mode {
                        NavigationMode::Parent => {
                            // User was navigating parents - select parent at cursor
                            parent_ids.get(state.parent_cursor_index).cloned()
                        },
                        NavigationMode::Reply => {
                            // User was navigating replies - select reply at cursor
                            reply_ids.get(state.reply_cursor_index).cloned()
                        },
                        NavigationMode::None => {
                            // No navigation yet - default to first reply
                            reply_ids.first().cloned()
                        }
                    };

                    if let Some(target_id) = target_post {
                        state.selected_post = Some(target_id.clone());
                        state.secondary_selected_post = None;
                        state.parent_cursor_index = 0;
                        state.reply_cursor_index = 0;
                        state.last_navigation_mode = NavigationMode::None; // Reset after selection
                        center_on = Some(target_id);
                        handled = true;
                    }
                }
            }

            // VIM-STYLE PARENT NAVIGATION (u, i, o, p keys)
            // u, p = move cursor left/right through parent IDs
            // i = select parent at cursor (secondary selection)

            // U: Move parent cursor left
            if consume_key(ctx, egui::Key::U, egui::Modifiers::NONE) {
                if has_parents {
                    if state.parent_cursor_index == 0 {
                        state.parent_cursor_index = parent_ids.len() - 1;
                    } else {
                        state.parent_cursor_index -= 1;
                    }
                    state.last_navigation_mode = crate::app::state::NavigationMode::Parent;
                    handled = true;
                }
            }

            // P: Move parent cursor right
            if consume_key(ctx, egui::Key::P, egui::Modifiers::NONE) {
                if has_parents {
                    state.parent_cursor_index = (state.parent_cursor_index + 1) % parent_ids.len();
                    state.last_navigation_mode = crate::app::state::NavigationMode::Parent;
                    handled = true;
                }
            }

            // I: Select parent at cursor (secondary selection + center viewport)
            if consume_key(ctx, egui::Key::I, egui::Modifiers::NONE) {
                if let Some(parent_id) = parent_ids.get(state.parent_cursor_index).cloned() {
                    state.secondary_selected_post = Some(parent_id.clone());
                    state.last_navigation_mode = crate::app::state::NavigationMode::Parent;
                    center_viewport_on_post(state, &parent_id, ctx);
                    handled = true;
                }
            }

            // VIM-STYLE REPLY NAVIGATION (j, k, l, ; keys)
            // j, ; = move cursor left/right through reply IDs
            // k = select reply at cursor (secondary selection)

            // J: Move reply cursor left
            if consume_key(ctx, egui::Key::J, egui::Modifiers::NONE) {
                if has_replies {
                    if state.reply_cursor_index == 0 {
                        state.reply_cursor_index = reply_ids.len() - 1;
                    } else {
                        state.reply_cursor_index -= 1;
                    }
                    state.last_navigation_mode = crate::app::state::NavigationMode::Reply;
                    handled = true;
                }
            }

            // Semicolon: Move reply cursor right
            if consume_key(ctx, egui::Key::Semicolon, egui::Modifiers::NONE) {
                if has_replies {
                    state.reply_cursor_index = (state.reply_cursor_index + 1) % reply_ids.len();
                    state.last_navigation_mode = crate::app::state::NavigationMode::Reply;
                    handled = true;
                }
            }

            // K: Select reply at cursor (secondary selection + center viewport)
            if consume_key(ctx, egui::Key::K, egui::Modifiers::NONE) {
                if let Some(reply_id) = reply_ids.get(state.reply_cursor_index).cloned() {
                    state.secondary_selected_post = Some(reply_id.clone());
                    state.last_navigation_mode = crate::app::state::NavigationMode::Reply;
                    center_viewport_on_post(state, &reply_id, ctx);
                    handled = true;
                }
            }

            // O: Move parent cursor right (alternative to P for vim users)
            if consume_key(ctx, egui::Key::O, egui::Modifiers::NONE) {
                if has_parents {
                    state.parent_cursor_index = (state.parent_cursor_index + 1) % parent_ids.len();
                    state.last_navigation_mode = crate::app::state::NavigationMode::Parent;
                    handled = true;
                }
            }

            // L: Move reply cursor right (alternative to semicolon for vim users)
            if consume_key(ctx, egui::Key::L, egui::Modifiers::NONE) {
                if has_replies {
                    state.reply_cursor_index = (state.reply_cursor_index + 1) % reply_ids.len();
                    state.last_navigation_mode = crate::app::state::NavigationMode::Reply;
                    handled = true;
                }
            }

            // RADIAL VIEW ARROW KEY NAVIGATION
            // Build data structures from radial_nodes directly (not posts) to avoid borrow conflicts
            if state.display_mode == ThreadDisplayMode::Radial {
                // Build ring groupings from radial_nodes
                let mut posts_by_ring: std::collections::HashMap<usize, Vec<(String, f32)>> = std::collections::HashMap::new();
                for (post_id, node) in &state.radial_nodes {
                    posts_by_ring.entry(node.ring).or_default().push((post_id.clone(), node.angle));
                }
                // Sort posts within each ring by angle
                for posts_in_ring in posts_by_ring.values_mut() {
                    posts_in_ring.sort_by(|a, b| {
                        a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal)
                    });
                }

                let current_ring = state.radial_nodes.get(&current_id).map(|n| n.ring).unwrap_or(0);
                let current_angle = state.radial_nodes.get(&current_id).map(|n| n.angle).unwrap_or(0.0);
                let max_ring = state.radial_nodes.values().map(|n| n.ring).max().unwrap_or(0);

                // LEFT: Previous post on same ring
                if consume_key(ctx, egui::Key::ArrowLeft, egui::Modifiers::NONE) {
                    if let Some(ring_posts) = posts_by_ring.get(&current_ring) {
                        if let Some(idx) = ring_posts.iter().position(|(id, _)| id == &current_id) {
                            let new_idx = if idx == 0 { ring_posts.len() - 1 } else { idx - 1 };
                            if let Some((new_id, _)) = ring_posts.get(new_idx) {
                                state.selected_post = Some(new_id.clone());
                                state.secondary_selected_post = None;
                                center_on = Some(new_id.clone());
                                handled = true;
                            }
                        }
                    }
                }

                // RIGHT: Next post on same ring
                if consume_key(ctx, egui::Key::ArrowRight, egui::Modifiers::NONE) {
                    if let Some(ring_posts) = posts_by_ring.get(&current_ring) {
                        if let Some(idx) = ring_posts.iter().position(|(id, _)| id == &current_id) {
                            let new_idx = (idx + 1) % ring_posts.len();
                            if let Some((new_id, _)) = ring_posts.get(new_idx) {
                                state.selected_post = Some(new_id.clone());
                                state.secondary_selected_post = None;
                                center_on = Some(new_id.clone());
                                handled = true;
                            }
                        }
                    }
                }

                // UP: Move to inner ring (lower ring number)
                if consume_key(ctx, egui::Key::ArrowUp, egui::Modifiers::NONE) {
                    if current_ring > 0 {
                        let target_ring = current_ring - 1;
                        if let Some(ring_posts) = posts_by_ring.get(&target_ring) {
                            // Find the post on the target ring closest in angle to current
                            let closest = ring_posts.iter()
                                .min_by(|(_, angle_a), (_, angle_b)| {
                                    let diff_a = (*angle_a - current_angle).abs();
                                    let diff_b = (*angle_b - current_angle).abs();
                                    diff_a.partial_cmp(&diff_b).unwrap_or(std::cmp::Ordering::Equal)
                                });
                            if let Some((new_id, _)) = closest {
                                state.selected_post = Some(new_id.clone());
                                state.secondary_selected_post = None;
                                center_on = Some(new_id.clone());
                                handled = true;
                            }
                        }
                    }
                }

                // DOWN: Move to outer ring (higher ring number)
                if consume_key(ctx, egui::Key::ArrowDown, egui::Modifiers::NONE) {
                    if current_ring < max_ring {
                        let target_ring = current_ring + 1;
                        if let Some(ring_posts) = posts_by_ring.get(&target_ring) {
                            // Find the post on the target ring closest in angle to current
                            let closest = ring_posts.iter()
                                .min_by(|(_, angle_a), (_, angle_b)| {
                                    let diff_a = (*angle_a - current_angle).abs();
                                    let diff_b = (*angle_b - current_angle).abs();
                                    diff_a.partial_cmp(&diff_b).unwrap_or(std::cmp::Ordering::Equal)
                                });
                            if let Some((new_id, _)) = closest {
                                state.selected_post = Some(new_id.clone());
                                state.secondary_selected_post = None;
                                center_on = Some(new_id.clone());
                                handled = true;
                            }
                        }
                    }
                }
            }

            // Apply viewport centering after releasing posts borrow
            if let Some(post_id) = center_on {
                center_viewport_on_post(state, &post_id, ctx);
            }

            if handled {
                ctx.request_repaint();
            }
        }
    }
}

/// Check if a TextEdit widget (like the draft post window) has focus
fn is_text_edit_focused(ctx: &egui::Context) -> bool {
    ctx.memory(|mem| {
        if let Some(focused_id) = mem.focused() {
            // Check if this ID corresponds to a TextEdit by looking at its ID string
            // Draft post TextEdit typically has "Draft Post" in its window ID chain
            let id_str = format!("{:?}", focused_id);
            return id_str.contains("Draft Post") || id_str.contains("text_edit") || id_str.contains("TextEdit");
        }
        false
    })
}

/// Check if user pressed a navigation key that should auto-select the OP
fn should_auto_select_op(ctx: &egui::Context) -> bool {
    ctx.input(|i| {
        i.key_pressed(egui::Key::Tab) ||
        i.key_pressed(egui::Key::Space) ||
        i.key_pressed(egui::Key::U) ||
        i.key_pressed(egui::Key::I) ||
        i.key_pressed(egui::Key::O) ||
        i.key_pressed(egui::Key::P) ||
        i.key_pressed(egui::Key::J) ||
        i.key_pressed(egui::Key::K) ||
        i.key_pressed(egui::Key::L) ||
        i.key_pressed(egui::Key::Semicolon) ||
        i.key_pressed(egui::Key::ArrowUp) ||
        i.key_pressed(egui::Key::ArrowDown) ||
        i.key_pressed(egui::Key::ArrowLeft) ||
        i.key_pressed(egui::Key::ArrowRight)
    })
}

/// Consume all navigation keys to prevent them from doing anything else this frame
fn consume_navigation_keys(ctx: &egui::Context) {
    ctx.input_mut(|i| {
        i.consume_key(egui::Modifiers::NONE, egui::Key::Tab);
        i.consume_key(egui::Modifiers::SHIFT, egui::Key::Tab);
        i.consume_key(egui::Modifiers::NONE, egui::Key::Space);
        i.consume_key(egui::Modifiers::NONE, egui::Key::U);
        i.consume_key(egui::Modifiers::NONE, egui::Key::I);
        i.consume_key(egui::Modifiers::NONE, egui::Key::O);
        i.consume_key(egui::Modifiers::NONE, egui::Key::P);
        i.consume_key(egui::Modifiers::NONE, egui::Key::J);
        i.consume_key(egui::Modifiers::NONE, egui::Key::K);
        i.consume_key(egui::Modifiers::NONE, egui::Key::L);
        i.consume_key(egui::Modifiers::NONE, egui::Key::Semicolon);
        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp);
        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown);
        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft);
        i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight);
    });
}

/// Consume a specific key with modifiers and return true if it was pressed
fn consume_key(ctx: &egui::Context, key: egui::Key, modifiers: egui::Modifiers) -> bool {
    let mut consumed = false;
    ctx.input_mut(|i| {
        if i.consume_key(modifiers, key) {
            consumed = true;
        }
    });
    consumed
}

/// Center the viewport on a specific post based on the current display mode
fn center_viewport_on_post(state: &mut ThreadState, post_id: &str, ctx: &egui::Context) {
    match state.display_mode {
        ThreadDisplayMode::Graph => {
            if let Some(node) = state.graph_nodes.get(post_id) {
                let zoom = state.graph_zoom;
                let scale = 100.0; // Graph view uses a 100.0 scale factor
                // Graph view adds rect.center() to the offset
                // ScreenPos = Center + Offset + Pos * Zoom * Scale
                // To center: Offset = -(Pos * Zoom * Scale)
                state.graph_offset = -(node.pos.to_vec2() * zoom * scale);
            }
        },
        ThreadDisplayMode::Sugiyama => {
            if let Some(node) = state.sugiyama_nodes.get(post_id) {
                let zoom = state.graph_zoom;
                // Sugiyama view adds rect.center() to the offset
                // ScreenPos = Center + Offset + Pos * Zoom
                // To center: Offset = -(Pos * Zoom)
                state.graph_offset = -(node.pos.to_vec2() * zoom);
            }
        },
        ThreadDisplayMode::Chronological => {
            if let Some(node) = state.chronological_nodes.get(post_id) {
                let zoom = state.graph_zoom;
                let screen_rect = ctx.screen_rect();
                let center_screen = screen_rect.center().to_vec2();
                // Chronological view adds rect.min (top-left) to the offset
                // ScreenPos = RectMin + Offset + Pos * Zoom
                // We want ScreenPos approx CenterScreen
                // Offset = CenterScreen - Pos * Zoom
                state.graph_offset = center_screen - (node.pos.to_vec2() * zoom);
            }
        },
        ThreadDisplayMode::Radial => {
            // For radial view, center viewport on the post using X/Y translation
            if let Some(node) = state.radial_nodes.get(post_id) {
                if node.ring == 0 {
                    // OP is at center, just reset offset
                    state.graph_offset = egui::vec2(0.0, 0.0);
                } else {
                    // Calculate post position relative to center
                    let ring_spacing = 400.0; // Must match RING_SPACING in radial.rs
                    let center_radius = 200.0; // Must match CENTER_RADIUS in radial.rs
                    let radius = center_radius + (node.ring as f32) * ring_spacing;
                    let adjusted_angle = node.angle + state.radial_rotation;

                    // Position relative to center (before zoom)
                    let pos_x = radius * adjusted_angle.cos();
                    let pos_y = radius * adjusted_angle.sin();

                    // Set offset to center on this position (scaled by zoom)
                    let zoom = state.graph_zoom;
                    state.graph_offset = egui::vec2(-pos_x * zoom, -pos_y * zoom);
                }
            }
        },
        _ => {}
    }
}
