use eframe::egui::{self, Context};
use crate::app::GraphchanApp;
use rfd::FileDialog;

pub fn render_identity_drawer(app: &mut GraphchanApp, ctx: &Context) {
    if !app.show_identity {
        return;
    }

    egui::SidePanel::right("identity_drawer")
        .resizable(true)
        .default_width(300.0)
        .show(ctx, |ui| {
            ui.heading("Identity Management");
            ui.add_space(10.0);

            let mut next_action = None;
            let mut trigger_load = false;

            enum Action {
                DownloadAvatar(String, String),
                UploadAvatar(String),
                PickAvatar,
            }

            // Scope 1: Identity Info
            {
                let state = &mut app.identity_state;
                let mut back_clicked = false;
                
                if let Some(peer) = &state.inspected_peer {
                    ui.heading("Peer Profile");
                    if ui.button("Back to My Identity").clicked() {
                        back_clicked = true;
                    }
                    ui.add_space(10.0);
                    
                    ui.group(|ui| {
                        if let Some(username) = &peer.username {
                            ui.label(format!("Username: {}", username));
                        }
                        if let Some(bio) = &peer.bio {
                            ui.label(format!("Bio: {}", bio));
                        }
                        ui.label(format!("Friendcode: {}", peer.friendcode.as_deref().unwrap_or("Unknown")));
                        ui.label(format!("Fingerprint: {}", peer.gpg_fingerprint.as_deref().unwrap_or("Unknown")));
                        
                        ui.add_space(10.0);
                        ui.label("Avatar:");
                        if let Some(avatar_id) = &peer.avatar_file_id {
                            if let Some(texture) = app.image_textures.get(avatar_id) {
                                 ui.add(egui::Image::from_texture(texture).max_width(150.0));
                            } else if let Some(pending) = app.image_pending.remove(avatar_id) {
                                let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
                                let tex = ui.ctx().load_texture(avatar_id, color, egui::TextureOptions::default());
                                app.image_textures.insert(avatar_id.clone(), tex.clone());
                                ui.add(egui::Image::from_texture(&tex).max_width(150.0));
                            } else if let Some(err) = app.image_errors.get(avatar_id) {
                                 ui.colored_label(egui::Color32::RED, "Error loading avatar");
                                 ui.label(err);
                            } else {
                                if !app.image_loading.contains(avatar_id) {
                                     let url = crate::app::resolve_blob_url(&app.base_url_input, avatar_id);
                                     next_action = Some(Action::DownloadAvatar(avatar_id.clone(), url));
                                }
                                ui.spinner();
                            }
                        } else {
                            ui.label("No avatar set.");
                        }
                    });
                } else if let Some(peer) = &state.local_peer {
                    if !state.initialized_inputs {
                        state.username_input = peer.username.clone().unwrap_or_default();
                        state.bio_input = peer.bio.clone().unwrap_or_default();
                        state.initialized_inputs = true;
                    }

                    ui.group(|ui| {
                        ui.label(format!("Friendcode: {}", peer.friendcode.as_deref().unwrap_or("Unknown")));
                        ui.label(format!("Fingerprint: {}", peer.gpg_fingerprint.as_deref().unwrap_or("Unknown")));
                        
                        ui.add_space(10.0);
                        ui.label("Username:");
                        ui.text_edit_singleline(&mut state.username_input);
                        
                        ui.add_space(5.0);
                        ui.label("Bio:");
                        ui.text_edit_multiline(&mut state.bio_input);

                        ui.add_space(10.0);
                        if ui.button("Save Profile").clicked() && !state.uploading {
                            state.uploading = true;
                            state.error = None;
                            let username = if state.username_input.is_empty() { None } else { Some(state.username_input.clone()) };
                            let bio = if state.bio_input.is_empty() { None } else { Some(state.bio_input.clone()) };
                            crate::app::tasks::update_profile(app.api.clone(), app.tx.clone(), username, bio);
                        }

                        ui.add_space(10.0);
                        ui.label("Current Avatar:");
                        if let Some(avatar_id) = &peer.avatar_file_id {
                            if let Some(texture) = app.image_textures.get(avatar_id) {
                                 ui.add(egui::Image::from_texture(texture).max_width(150.0));
                            } else if let Some(pending) = app.image_pending.remove(avatar_id) {
                                let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
                                let tex = ui.ctx().load_texture(avatar_id, color, egui::TextureOptions::default());
                                app.image_textures.insert(avatar_id.clone(), tex.clone());
                                ui.add(egui::Image::from_texture(&tex).max_width(150.0));
                            } else if let Some(err) = app.image_errors.get(avatar_id) {
                                 ui.colored_label(egui::Color32::RED, "Error loading avatar");
                                 ui.label(err);
                            } else {
                                if !app.image_loading.contains(avatar_id) {
                                     let url = crate::app::resolve_blob_url(&app.base_url_input, avatar_id);
                                     next_action = Some(Action::DownloadAvatar(avatar_id.clone(), url));
                                }
                                ui.spinner();
                            }
                        } else {
                            ui.label("No avatar set.");
                        }
                    });

                    ui.add_space(20.0);
                    ui.separator();
                    ui.add_space(20.0);
        
                    ui.heading("Update Avatar");
                    
                    // Scope 2: Update Avatar
                    // We need to access state again, but we are in a mutable borrow.
                    // Actually we are inside the `show` closure, so `app` is borrowed mutably by `render_identity_drawer`,
                    // and `state` is borrowed mutably from `app`.
                    // We can just continue using `state`.
                    
                    if let Some(path) = &state.avatar_path {
                        ui.label(format!("Selected file: {}", path));
                    }
    
                    if ui.button("Select Image...").clicked() {
                        next_action = Some(Action::PickAvatar);
                    }
    
                    if let Some(path) = &state.avatar_path {
                        if ui.button("Upload Avatar").clicked() && !state.uploading {
                            state.uploading = true;
                            state.error = None;
                            next_action = Some(Action::UploadAvatar(path.clone()));
                        }
                    }
    
                    if state.uploading {
                        ui.spinner();
                        ui.label("Uploading...");
                    }
    
                    if let Some(err) = &state.error {
                        ui.colored_label(egui::Color32::RED, err);
                    }
                } else {
                    ui.spinner();
                    ui.label("Loading identity...");
                    if state.local_peer.is_none() && state.error.is_none() {
                         trigger_load = true;
                    }
                }
                
                if back_clicked {
                    state.inspected_peer = None;
                }
            }

            if trigger_load {
                app.spawn_load_identity();
            }



            match next_action {
                Some(Action::DownloadAvatar(id, url)) => {
                    app.spawn_load_image(&id, &url);
                }
                Some(Action::UploadAvatar(path)) => {
                    crate::app::tasks::upload_avatar(
                        app.api.clone(),
                        app.tx.clone(),
                        std::path::PathBuf::from(path).to_string_lossy().to_string(),
                    );
                }
                Some(Action::PickAvatar) => {
                    if let Some(path) = FileDialog::new()
                        .add_filter("image", &["png", "jpg", "jpeg", "gif", "webp"])
                        .pick_file() 
                    {
                        app.identity_state.avatar_path = Some(path.display().to_string());
                        app.identity_state.error = None;
                    }
                }
                None => {}
            }
        });
}
