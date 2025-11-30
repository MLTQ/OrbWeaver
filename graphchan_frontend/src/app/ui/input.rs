use eframe::egui;
use crate::app::state::ThreadState;

pub fn handle_keyboard_input(state: &mut ThreadState, ui: &mut egui::Ui) {
    log::info!("Checking keyboard input (Mode: {:?})", state.display_mode);
    if let Some(details) = &state.details {
            let posts = &details.posts;
            
            // If no post is selected, maybe select the first one (OP)?
            // Or just do nothing.
            let current_id = match &state.selected_post {
                Some(id) => id.clone(),
                None => {
                    log::info!("No post selected");
                    return;
                },
            };

            // Find current post
            let current_post = posts.iter().find(|p| p.id == current_id);
            if current_post.is_none() {
                log::warn!("Selected post {} not found in details", current_id);
                return;
            }
            let current_post = current_post.unwrap();

            // Calculate replies (children)
            // We do this every frame/input, which is fine for N < 1000
            let mut replies: Vec<&crate::models::PostView> = posts.iter()
                .filter(|p| p.parent_post_ids.contains(&current_id))
                .collect();
            // Sort by created_at or ID to ensure stable order
            replies.sort_by_key(|p| &p.created_at);
            
            log::info!("Current: {}, Replies: {}", current_id, replies.len());

            // Diagnostic: Log any key press
            ui.input(|i| {
                for event in &i.events {
                    if let egui::Event::Key { key, pressed: true, .. } = event {
                        log::info!("Raw Event: Key {:?} pressed", key);
                    }
                }
            });
            
            if let Some(focus_id) = ui.ctx().memory(|m| m.focused()) {
                // log::info!("Widget with focus: {:?}", focus_id);
            } else {
                // log::info!("No widget has focus");
            }

            let mut handled = false;

            // UP: Go to parent
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                log::info!("Key Up pressed");
                if let Some(parent_id) = current_post.parent_post_ids.first() {
                    log::info!("Navigating to parent: {}", parent_id);
                    state.selected_post = Some(parent_id.clone());
                    state.secondary_selected_post = None;
                    state.focused_link_index = None;
                    handled = true;
                }
            }

            // DOWN: Peek at reply (Secondary Highlight)
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
                log::info!("Key Down pressed");
                if let Some(index) = state.focused_link_index {
                    if index < replies.len() {
                        let reply = replies[index];
                        log::info!("Peeking reply: {}", reply.id);
                        state.secondary_selected_post = Some(reply.id.clone());
                        handled = true;
                    }
                } else if !replies.is_empty() {
                    // If no link focused, focus first one and peek
                    log::info!("Peeking first reply: {}", replies[0].id);
                    state.focused_link_index = Some(0);
                    state.secondary_selected_post = Some(replies[0].id.clone());
                    handled = true;
                } else {
                    log::info!("No replies to peek");
                }
            }

            // RIGHT: Next reply link
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
                log::info!("Key Right pressed");
                if !replies.is_empty() {
                    let new_index = match state.focused_link_index {
                        Some(i) => (i + 1) % replies.len(),
                        None => 0,
                    };
                    log::info!("Focused link index: {}", new_index);
                    state.focused_link_index = Some(new_index);
                    
                    // If we already have a secondary selection, update it to follow the link
                    if state.secondary_selected_post.is_some() {
                        state.secondary_selected_post = Some(replies[new_index].id.clone());
                    }
                    handled = true;
                } else {
                    log::info!("No replies to cycle");
                }
            }

            // LEFT: Previous reply link
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
                log::info!("Key Left pressed");
                if !replies.is_empty() {
                    let new_index = match state.focused_link_index {
                        Some(i) => if i == 0 { replies.len() - 1 } else { i - 1 },
                        None => replies.len() - 1,
                    };
                    log::info!("Focused link index: {}", new_index);
                    state.focused_link_index = Some(new_index);

                    // If we already have a secondary selection, update it to follow the link
                    if state.secondary_selected_post.is_some() {
                        state.secondary_selected_post = Some(replies[new_index].id.clone());
                    }
                    handled = true;
                }
            }

            // SPACE: Select focused reply (Primary Highlight)
            if ui.input(|i| i.key_pressed(egui::Key::Space)) {
                log::info!("Key Space pressed");
                if let Some(secondary) = &state.secondary_selected_post {
                    log::info!("Selecting secondary: {}", secondary);
                    state.selected_post = Some(secondary.clone());
                    state.secondary_selected_post = None;
                    state.focused_link_index = None;
                    handled = true;
                }
            }
            
            // Number Keys (1-9, 0)
            let number_keys = [
                (egui::Key::Num1, 0), (egui::Key::Num2, 1), (egui::Key::Num3, 2),
                (egui::Key::Num4, 3), (egui::Key::Num5, 4), (egui::Key::Num6, 5),
                (egui::Key::Num7, 6), (egui::Key::Num8, 7), (egui::Key::Num9, 8),
                (egui::Key::Num0, 9),
            ];

            for (key, index) in number_keys {
                if ui.input(|i| i.key_pressed(key)) {
                    let modifiers = ui.input(|i| i.modifiers);
                    if modifiers.command {
                        // Cmd + Number: Go to Nth Parent
                        if index < current_post.parent_post_ids.len() {
                            let parent_id = &current_post.parent_post_ids[index];
                            log::info!("Navigating to parent {}: {}", index + 1, parent_id);
                            state.selected_post = Some(parent_id.clone());
                            state.secondary_selected_post = None;
                            state.focused_link_index = None;
                            handled = true;
                        }
                    } else {
                        // Number: Go to Nth Reply
                        if index < replies.len() {
                            let reply = replies[index];
                            log::info!("Navigating to reply {}: {}", index + 1, reply.id);
                            state.selected_post = Some(reply.id.clone());
                            state.secondary_selected_post = None;
                            state.focused_link_index = None;
                            handled = true;
                        }
                    }
                }
            }

            if handled {
                // Center camera if selection changed
                let target_id = state.secondary_selected_post.as_ref().or(state.selected_post.as_ref());
                if let Some(id) = target_id {
                    use crate::app::state::ThreadDisplayMode;
                    let node = match state.display_mode {
                        ThreadDisplayMode::Graph => state.graph_nodes.get(id),
                        ThreadDisplayMode::Chronological => state.chronological_nodes.get(id),
                        ThreadDisplayMode::Sugiyama => state.sugiyama_nodes.get(id),
                        _ => None,
                    };

                    if let Some(node) = node {
                        let viewport = ui.clip_rect();
                        // Target position in world space
                        let target_pos = node.pos;
                        let zoom = state.graph_zoom;
                        
                        // We want the node to be at the center of the viewport
                        // ScreenPos = Center + Offset + WorldPos * Zoom
                        // Center = Center + Offset + WorldPos * Zoom
                        // Offset = -WorldPos * Zoom
                        
                        // Smooth transition? For now, instant jump.
                        state.graph_offset = -target_pos.to_vec2() * zoom;
                    }
                }
                
                ui.ctx().request_repaint();
            }
    }
}
