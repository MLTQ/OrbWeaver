use eframe::egui::{self, Color32, RichText, ScrollArea};

use crate::app::{state::ConversationState, GraphchanApp};

pub fn render_conversation(app: &mut GraphchanApp, ui: &mut egui::Ui, state: &mut ConversationState) {
    ui.horizontal(|ui| {
        if ui.button("← Back to Private Threads").clicked() {
            app.view = crate::app::state::ViewState::Catalog;
        }

        ui.heading("Direct Message");

        if let Some(peer) = &state.peer_info {
            ui.label(RichText::new(format!(
                "with {}",
                peer.username.as_deref().or(peer.alias.as_deref()).unwrap_or(&peer.id)
            )).strong());
        } else {
            ui.label(RichText::new(&state.peer_id).monospace().size(10.0));
        }
    });

    ui.separator();

    // Messages area
    let messages_height = ui.available_height() - 100.0;

    ScrollArea::vertical()
        .max_height(messages_height)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            if state.messages_loading && state.messages.is_empty() {
                ui.add(egui::Spinner::new());
            }

            if let Some(err) = &state.messages_error {
                ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
            }

            for message in &state.messages {
                let is_outgoing = message.from_peer_id != state.peer_id;

                egui::Frame::none()
                    .inner_margin(egui::vec2(8.0, 4.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            if is_outgoing {
                                ui.add_space(50.0); // Indent outgoing messages
                            }

                            let frame_color = if is_outgoing {
                                ui.visuals().code_bg_color
                            } else {
                                ui.visuals().extreme_bg_color
                            };

                            egui::Frame::group(ui.style())
                                .fill(frame_color)
                                .inner_margin(egui::vec2(10.0, 8.0))
                                .show(ui, |ui| {
                                    ui.set_max_width(ui.available_width() - 50.0);

                                    ui.label(&message.body);

                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(&message.created_at).size(9.0).weak());
                                        if message.read_at.is_some() {
                                            ui.label(RichText::new("✓ Read").size(9.0).weak());
                                        }
                                    });
                                });

                            if !is_outgoing {
                                ui.add_space(50.0); // Indent incoming messages on right
                            }
                        });
                    });

                ui.add_space(4.0);
            }
        });

    ui.separator();

    // Send message input
    ui.horizontal(|ui| {
        let response = ui.add_sized(
            [ui.available_width() - 80.0, 40.0],
            egui::TextEdit::multiline(&mut state.new_message_body)
                .hint_text("Type a message...")
        );

        if ui.add_enabled(!state.sending, egui::Button::new("Send")).clicked()
            || (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter) && !i.modifiers.shift))
        {
            app.spawn_send_dm(state);
        }
    });

    if let Some(err) = &state.send_error {
        ui.colored_label(Color32::LIGHT_RED, format!("Error: {}", err));
    }

    if state.sending {
        ui.add(egui::Spinner::new());
    }
}

pub fn render_conversations_list(
    app: &mut GraphchanApp,
    ui: &mut egui::Ui,
    conversations: &[crate::models::ConversationView],
) {
    // Header with "New Message" button
    ui.horizontal(|ui| {
        ui.heading("Messages");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("+ New Message").clicked() {
                app.dm_state.show_new_conversation_picker = !app.dm_state.show_new_conversation_picker;
                app.dm_state.new_conversation_filter.clear();
            }
        });
    });

    // Peer picker (shown when toggled)
    let mut peer_to_open: Option<crate::models::PeerView> = None;

    if app.dm_state.show_new_conversation_picker {
        let existing_peer_ids: std::collections::HashSet<&str> = conversations
            .iter()
            .map(|c| c.peer_id.as_str())
            .collect();

        egui::Frame::group(ui.style())
            .inner_margin(egui::vec2(12.0, 8.0))
            .show(ui, |ui| {
                ui.label(RichText::new("Select a friend").strong());
                ui.add_space(4.0);

                ui.add(
                    egui::TextEdit::singleline(&mut app.dm_state.new_conversation_filter)
                        .hint_text("Filter friends...")
                );
                ui.add_space(4.0);

                let filter = app.dm_state.new_conversation_filter.to_lowercase();
                let mut any_shown = false;

                // Collect peers to avoid borrow issues
                let peers: Vec<_> = app.peers.values().cloned().collect();

                for peer in &peers {
                    // Filter by search text
                    let name = peer.username.as_deref()
                        .or(peer.alias.as_deref())
                        .unwrap_or(&peer.id);
                    if !filter.is_empty() && !name.to_lowercase().contains(&filter) {
                        continue;
                    }

                    any_shown = true;
                    let has_conversation = existing_peer_ids.contains(peer.id.as_str());

                    ui.horizontal(|ui| {
                        ui.label(name);
                        if has_conversation {
                            ui.label(RichText::new("(existing)").weak().size(10.0));
                        }
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Message").clicked() {
                                peer_to_open = Some(peer.clone());
                            }
                        });
                    });
                }

                if !any_shown {
                    if app.peers.is_empty() {
                        ui.label(RichText::new("No friends yet. Add peers from the Following page.").weak());
                    } else {
                        ui.label(RichText::new("No matching friends.").weak());
                    }
                }
            });

        ui.add_space(8.0);
    }

    // Open selected peer conversation (after UI rendering to avoid borrow conflict)
    if let Some(peer) = peer_to_open {
        app.dm_state.show_new_conversation_picker = false;
        app.dm_state.new_conversation_filter.clear();
        open_conversation_with_peer(app, peer);
        return;
    }

    ui.separator();

    if conversations.is_empty() {
        ui.add_space(20.0);
        ui.vertical_centered(|ui| {
            ui.label("No conversations yet.");
            ui.label(RichText::new("Use \"+ New Message\" above to start one.").weak());
        });
        return;
    }

    let mut conversation_to_open: Option<String> = None;

    ScrollArea::vertical().show(ui, |ui| {
        for conv in conversations {
            egui::Frame::group(ui.style())
                .fill(ui.visuals().extreme_bg_color)
                .inner_margin(egui::vec2(12.0, 8.0))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    ui.horizontal(|ui| {
                        let peer_name = conv.peer_username
                            .as_deref()
                            .or(conv.peer_alias.as_deref())
                            .unwrap_or(&conv.peer_id);

                        if ui.button(RichText::new(peer_name).strong()).clicked() {
                            conversation_to_open = Some(conv.peer_id.clone());
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if conv.unread_count > 0 {
                                ui.label(
                                    RichText::new(format!("{} unread", conv.unread_count))
                                        .color(Color32::from_rgb(100, 200, 100))
                                        .strong()
                                );
                            }
                        });
                    });

                    if let Some(preview) = &conv.last_message_preview {
                        ui.label(RichText::new(preview).size(11.0).weak());
                    }

                    if let Some(timestamp) = &conv.last_message_at {
                        ui.label(RichText::new(timestamp).size(9.0).weak());
                    }
                });

            ui.add_space(8.0);
        }
    });

    if let Some(peer_id) = conversation_to_open {
        open_conversation(app, peer_id);
    }
}

fn open_conversation(app: &mut GraphchanApp, peer_id: String) {
    // Find peer info
    let peer_info = app.peers.get(&peer_id).cloned();

    let mut state = ConversationState {
        peer_id: peer_id.clone(),
        peer_info,
        ..Default::default()
    };

    app.spawn_load_messages(&mut state);
    app.view = crate::app::state::ViewState::Conversation(state);
}

pub fn open_conversation_with_peer(app: &mut GraphchanApp, peer: crate::models::PeerView) {
    let mut state = ConversationState {
        peer_id: peer.id.clone(),
        peer_info: Some(peer),
        ..Default::default()
    };

    app.spawn_load_messages(&mut state);
    app.view = crate::app::state::ViewState::Conversation(state);
}
