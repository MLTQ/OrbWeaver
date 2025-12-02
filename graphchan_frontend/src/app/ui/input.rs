use eframe::egui;
use crate::app::GraphchanApp;
use crate::app::state::{ViewState, ThreadState};

pub fn handle_keyboard_input(app: &mut GraphchanApp, ctx: &egui::Context) {
    if let ViewState::Thread(state) = &mut app.view {
        if let Some(details) = &state.details {
            let posts = &details.posts;
            if posts.is_empty() { return; }

            // Helper to check and consume a key
            let mut consume = |key: egui::Key, modifiers: egui::Modifiers| -> bool {
                let mut consumed = false;
                ctx.input_mut(|i| {
                    if i.consume_key(modifiers, key) {
                        consumed = true;
                    }
                });
                
                // If we consumed a key, also ensure we surrender focus so egui doesn't try to cycle
                if consumed {
                    ctx.memory_mut(|m| {
                        if let Some(id) = m.focused() {
                            m.surrender_focus(id);
                        }
                    });
                    
                    // Also filter the event to be absolutely sure
                    ctx.input_mut(|i| {
                        i.events.retain(|e| {
                            match e {
                                egui::Event::Key { key: k, .. } => *k != key,
                                _ => true,
                            }
                        });
                    });
                }
                consumed
            };

            // Check for any relevant key press to auto-select OP if needed
            // We peek at input first without consuming to see if we need to init selection
            let relevant_keys = [
                egui::Key::ArrowUp, egui::Key::ArrowDown, 
                egui::Key::ArrowLeft, egui::Key::ArrowRight, 
                egui::Key::Space, egui::Key::Tab
            ];
            
            let any_pressed = ctx.input(|i| relevant_keys.iter().any(|k| i.key_pressed(*k)));

            if !any_pressed {
                return;
            }

            // If nothing selected, select first post (OP) and consume the triggering key
            let current_id = match &state.selected_post {
                Some(id) => id.clone(),
                None => {
                    if let Some(first) = posts.first() {
                        state.selected_post = Some(first.id.clone());
                        // Consume all relevant keys to prevent them from doing anything else this frame
                        ctx.input_mut(|i| {
                            for k in relevant_keys { 
                                i.consume_key(egui::Modifiers::NONE, k); 
                                i.consume_key(egui::Modifiers::SHIFT, k);
                            }
                        });
                        ctx.request_repaint();
                    }
                    return;
                }
            };

            // Find current post index and object
            let current_index = posts.iter().position(|p| p.id == current_id);
            let current_post = match current_index.and_then(|i| posts.get(i)) {
                Some(p) => p,
                None => return,
            };

            let mut handled = false;

            // TAB: Next Post
            if consume(egui::Key::Tab, egui::Modifiers::NONE) {
                if let Some(idx) = current_index {
                    let new_index = (idx + 1) % posts.len();
                    if let Some(next_post) = posts.get(new_index) {
                        state.selected_post = Some(next_post.id.clone());
                        state.secondary_selected_post = None;
                        state.focused_link_index = None;
                        handled = true;
                    }
                }
            }
            // SHIFT+TAB: Previous Post
            else if consume(egui::Key::Tab, egui::Modifiers::SHIFT) {
                if let Some(idx) = current_index {
                    let new_index = if idx == 0 { posts.len() - 1 } else { idx - 1 };
                    if let Some(next_post) = posts.get(new_index) {
                        state.selected_post = Some(next_post.id.clone());
                        state.secondary_selected_post = None;
                        state.focused_link_index = None;
                        handled = true;
                    }
                }
            }

            // Graph Navigation Logic
            // Calculate replies (children)
            let mut replies: Vec<&crate::models::PostView> = posts.iter()
                .filter(|p| p.parent_post_ids.contains(&current_id))
                .collect();
            replies.sort_by_key(|p| &p.created_at);

            // UP: Go to parent
            if consume(egui::Key::ArrowUp, egui::Modifiers::NONE) {
                if let Some(parent_id) = current_post.parent_post_ids.first() {
                    state.selected_post = Some(parent_id.clone());
                    state.secondary_selected_post = None;
                    state.focused_link_index = None;
                    handled = true;
                }
            }

            // DOWN: Peek at reply (Secondary Highlight)
            if consume(egui::Key::ArrowDown, egui::Modifiers::NONE) {
                if let Some(index) = state.focused_link_index {
                    if index < replies.len() {
                        let reply = replies[index];
                        state.secondary_selected_post = Some(reply.id.clone());
                        handled = true;
                    }
                } else if !replies.is_empty() {
                    // If no link focused, focus first one and peek
                    state.focused_link_index = Some(0);
                    state.secondary_selected_post = Some(replies[0].id.clone());
                    handled = true;
                }
            }

            // RIGHT: Next reply link
            if consume(egui::Key::ArrowRight, egui::Modifiers::NONE) {
                if !replies.is_empty() {
                    let new_index = match state.focused_link_index {
                        Some(i) => (i + 1) % replies.len(),
                        None => 0,
                    };
                    state.focused_link_index = Some(new_index);
                    
                    // If we already have a secondary selection, update it to follow the link
                    if state.secondary_selected_post.is_some() {
                        state.secondary_selected_post = Some(replies[new_index].id.clone());
                    }
                    handled = true;
                }
            }

            // LEFT: Previous reply link
            if consume(egui::Key::ArrowLeft, egui::Modifiers::NONE) {
                if !replies.is_empty() {
                    let new_index = match state.focused_link_index {
                        Some(i) => if i == 0 { replies.len() - 1 } else { i - 1 },
                        None => replies.len() - 1,
                    };
                    state.focused_link_index = Some(new_index);

                    // If we already have a secondary selection, update it to follow the link
                    if state.secondary_selected_post.is_some() {
                        state.secondary_selected_post = Some(replies[new_index].id.clone());
                    }
                    handled = true;
                }
            }

            // SPACE: Select focused reply (Primary Highlight)
            if consume(egui::Key::Space, egui::Modifiers::NONE) {
                if let Some(secondary) = &state.secondary_selected_post {
                    state.selected_post = Some(secondary.clone());
                    state.secondary_selected_post = None;
                    state.focused_link_index = None;
                    handled = true;
                }
            }
            
            if handled {
                // Center camera if selection changed
                let target_id = state.secondary_selected_post.as_ref().or(state.selected_post.as_ref());
                if let Some(id) = target_id {
                    use crate::app::state::ThreadDisplayMode;
                    
                    // Different views use different coordinate systems
                    match state.display_mode {
                        ThreadDisplayMode::Graph => {
                            if let Some(node) = state.graph_nodes.get(id) {
                                let zoom = state.graph_zoom;
                                let scale = 100.0; // Graph view uses a 100.0 scale factor
                                // Graph view adds rect.center() to the offset
                                // ScreenPos = Center + Offset + Pos * Zoom * Scale
                                // To center: Offset = -(Pos * Zoom * Scale)
                                state.graph_offset = -(node.pos.to_vec2() * zoom * scale);
                            }
                        },
                        ThreadDisplayMode::Sugiyama => {
                            if let Some(node) = state.sugiyama_nodes.get(id) {
                                let zoom = state.graph_zoom;
                                // Sugiyama view adds rect.center() to the offset
                                // ScreenPos = Center + Offset + Pos * Zoom
                                // To center: Offset = -(Pos * Zoom)
                                state.graph_offset = -(node.pos.to_vec2() * zoom);
                            }
                        },
                        ThreadDisplayMode::Chronological => {
                            if let Some(node) = state.chronological_nodes.get(id) {
                                let zoom = state.graph_zoom;
                                let screen_rect = ctx.screen_rect();
                                let center_screen = screen_rect.center().to_vec2();
                                // Chronological view adds rect.min (top-left) to the offset
                                // ScreenPos = RectMin + Offset + Pos * Zoom
                                // We want ScreenPos approx CenterScreen
                                // Offset = CenterScreen - RectMin - Pos * Zoom
                                // We approximate RectMin as (0, 0) or ignore it since CenterScreen is relative to window
                                // Actually, if we set Offset = CenterScreen - Pos * Zoom, then:
                                // ScreenPos = RectMin + CenterScreen - Pos * Zoom + Pos * Zoom = RectMin + CenterScreen
                                // This puts it slightly below center (by RectMin.y), which is fine.
                                state.graph_offset = center_screen - (node.pos.to_vec2() * zoom);
                            }
                        },
                        _ => {}
                    }
                }
                
                ctx.request_repaint();
            }
        }
    }
}
