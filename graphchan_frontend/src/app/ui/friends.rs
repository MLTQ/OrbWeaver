use eframe::egui;
use crate::app::GraphchanApp;

pub fn render_friends_page(app: &mut GraphchanApp, ui: &mut egui::Ui) {
    ui.heading("Following Management");
    ui.add_space(20.0);

    let mut peer_to_unfollow: Option<String> = None;
    let mut peer_to_message: Option<crate::models::PeerView> = None;

    // Add Peer Section
    egui::Frame::group(ui.style())
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.heading("Follow a Peer");
            ui.add_space(10.0);

            ui.label("Enter a friendcode to follow a peer:");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut app.identity_state.friendcode_input);

                if ui.button("Follow").clicked() 
                    && !app.identity_state.adding_peer 
                    && !app.identity_state.friendcode_input.trim().is_empty() 
                {
                    app.identity_state.adding_peer = true;
                    app.identity_state.error = None;
                    let friendcode = app.identity_state.friendcode_input.trim().to_string();
                    crate::app::tasks::add_peer(
                        app.api.clone(),
                        app.tx.clone(),
                        friendcode,
                    );
                }

                if app.identity_state.adding_peer {
                    ui.spinner();
                    ui.label("Adding...");
                }
            });

            if let Some(err) = &app.identity_state.error {
                ui.colored_label(egui::Color32::RED, err);
            }
        });

    ui.add_space(20.0);

    // Following List Section
    ui.heading("Following");
    ui.add_space(10.0);

    if app.peers.is_empty() {
        ui.label("You aren't following anyone yet.");
    } else {
        egui::Grid::new("friends_grid")
            .num_columns(3)
            .spacing(egui::vec2(20.0, 10.0))
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Username").strong());
                ui.label(egui::RichText::new("Peer ID").strong());
                ui.label(egui::RichText::new("Actions").strong());
                ui.end_row();

                for peer in app.peers.values() {
                    let username = peer.username.as_deref().unwrap_or("Unknown");

                    // Username (clickable)
                    if ui.link(username).clicked() {
                        app.view = crate::app::state::ViewState::FollowingCatalog(peer.clone());
                    }

                    // Peer ID (shorter than friendcode)
                    ui.label(egui::RichText::new(&peer.id).monospace().size(10.0));

                    // Actions
                    ui.horizontal(|ui| {
                        if ui.button("💬 Message").clicked() {
                            peer_to_message = Some(peer.clone());
                        }
                        if ui.button("View Profile").clicked() {
                            app.show_identity = true;
                            app.identity_state.inspected_peer = Some(peer.clone());
                        }
                        if ui.button("Unfollow").clicked() {
                            peer_to_unfollow = Some(peer.id.clone());
                        }
                    });

                    ui.end_row();
                }
            });
    }

    // Handle actions
    if let Some(peer_id) = peer_to_unfollow {
        crate::app::tasks::unfollow_peer(app.api.clone(), app.tx.clone(), peer_id);
    }

    if let Some(peer) = peer_to_message {
        super::conversations::open_conversation_with_peer(app, peer);
    }
}
