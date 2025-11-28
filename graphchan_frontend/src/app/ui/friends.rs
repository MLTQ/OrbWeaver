use eframe::egui;
use crate::app::GraphchanApp;

pub fn render_friends_page(app: &mut GraphchanApp, ui: &mut egui::Ui) {
    ui.heading("Friends Management");
    ui.add_space(20.0);

    // Add Friend Section
    egui::Frame::group(ui.style())
        .fill(ui.visuals().extreme_bg_color)
        .inner_margin(12.0)
        .show(ui, |ui| {
            ui.heading("Add Friend");
            ui.add_space(10.0);
            
            ui.label("Enter a friendcode to connect with a peer:");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut app.identity_state.friendcode_input);
                
                if ui.button("Add Friend").clicked() 
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

    // Friends List Section
    ui.heading("My Friends");
    ui.add_space(10.0);

    if app.peers.is_empty() {
        ui.label("You haven't added any friends yet.");
    } else {
        egui::Grid::new("friends_grid")
            .num_columns(3)
            .spacing(egui::vec2(20.0, 10.0))
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Username").strong());
                ui.label(egui::RichText::new("Friendcode").strong());
                ui.label(egui::RichText::new("Actions").strong());
                ui.end_row();

                for peer in app.peers.values() {
                    let username = peer.username.as_deref().unwrap_or("Unknown");
                    
                    // Username (clickable)
                    if ui.link(username).clicked() {
                        app.show_identity = true;
                        app.identity_state.inspected_peer = Some(peer.clone());
                    }

                    // Friendcode
                    ui.label(peer.friendcode.as_deref().unwrap_or("Unknown"));

                    // Actions
                    if ui.button("View Profile").clicked() {
                        app.show_identity = true;
                        app.identity_state.inspected_peer = Some(peer.clone());
                    }
                    
                    ui.end_row();
                }
            });
    }
}
