use eframe::egui;
use crate::app::GraphchanApp;
use crate::app::state::{ViewState, ThreadState};

pub fn handle_keyboard_input(app: &mut GraphchanApp, ui: &mut egui::Ui) {
    // Only proceed if any relevant key is pressed to avoid O(N) lookups every frame
    let relevant_keys = [
        egui::Key::ArrowUp, egui::Key::ArrowDown, 
        egui::Key::ArrowLeft, egui::Key::ArrowRight, 
        egui::Key::Space, egui::Key::Tab
    ];
    
    let key_pressed = ui.input(|i| relevant_keys.iter().any(|k| i.key_pressed(*k)));
    
    if !key_pressed {
        return;
    }

    if let ViewState::Thread(state) = &mut app.view {
        if let Some(details) = &state.details {
            let posts = &details.posts;
            
            let current_id = match &state.selected_post {
                Some(id) => id.clone(),
                None => return,
            };

            // Find current post
            let current_post = match posts.iter().find(|p| p.id == current_id) {
                Some(p) => p,
                None => return,
            };

            // Calculate replies (children)
            let mut replies: Vec<&crate::models::PostView> = posts.iter()
                .filter(|p| p.parent_post_ids.contains(&current_id))
                .collect();
            // Sort by created_at or ID to ensure stable order
            replies.sort_by_key(|p| &p.created_at);
            
            let mut handled = false;

            // UP: Go to parent
            if ui.input(|i| i.key_pressed(egui::Key::ArrowUp)) {
                if let Some(parent_id) = current_post.parent_post_ids.first() {
                    state.selected_post = Some(parent_id.clone());
                    state.secondary_selected_post = None;
                    state.focused_link_index = None;
                    handled = true;
                }
            }

            // DOWN: Peek at reply (Secondary Highlight)
            if ui.input(|i| i.key_pressed(egui::Key::ArrowDown)) {
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
            if ui.input(|i| i.key_pressed(egui::Key::ArrowRight)) {
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
            if ui.input(|i| i.key_pressed(egui::Key::ArrowLeft)) {
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
            if ui.input(|i| i.key_pressed(egui::Key::Space)) {
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
                    let node = match state.display_mode {
                        ThreadDisplayMode::Graph => state.graph_nodes.get(id),
                        ThreadDisplayMode::Chronological => state.chronological_nodes.get(id),
                        ThreadDisplayMode::Sugiyama => state.sugiyama_nodes.get(id),
                        _ => None,
                    };

                    if let Some(node) = node {
                        // Target position in world space
                        let target_pos = node.pos;
                        let zoom = state.graph_zoom;
                        
                        // We want the node to be at the center of the viewport
                        // ScreenPos = Center + Offset + WorldPos * Zoom
                        // Center = Center + Offset + WorldPos * Zoom
                        // Offset = -WorldPos * Zoom
                        
                        state.graph_offset = -target_pos.to_vec2() * zoom;
                    }
                }
                
                ui.ctx().request_repaint();
            }
        }
    }
}
