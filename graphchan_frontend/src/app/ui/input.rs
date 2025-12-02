use eframe::egui;
use crate::app::GraphchanApp;
use crate::app::state::{ViewState, ThreadState};

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
            if consume_key(ctx, egui::Key::Space, egui::Modifiers::NONE) {
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
        i.key_pressed(egui::Key::Semicolon)
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
    use crate::app::state::ThreadDisplayMode;

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
        _ => {}
    }
}
