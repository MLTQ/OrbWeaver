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
            let mut next_action = None;
            let mut trigger_load = false;

            enum Action {
                DownloadAvatar(String, String),
                UploadAvatar(String),
                PickAvatar,
                AddPeer(String),
            }

            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Identity Management");
                ui.add_space(10.0);

                // Scope 1: Identity Info
                {
                let state = &mut app.identity_state;
                let mut back_clicked = false;
                
                if let Some(peer) = &state.inspected_peer {
                    ui.heading("Peer Profile");
                    if ui.button("← Back to My Identity").clicked() {
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

                        // Follow button
                        if let Some(friendcode) = &peer.friendcode {
                            // Check if we already have this peer in our list
                            let already_following = app.peers.contains_key(&peer.id);

                            if already_following {
                                ui.label("✓ Following");
                            } else {
                                if ui.button("Follow").clicked() && !state.adding_peer {
                                    state.adding_peer = true;
                                    state.error = None;
                                    next_action = Some(Action::AddPeer(friendcode.clone()));
                                }
                            }
                        }

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

                    ui.add_space(20.0);

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
            }); // End ScrollArea

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
                Some(Action::AddPeer(friendcode)) => {
                    crate::app::tasks::add_peer(
                        app.api.clone(),
                        app.tx.clone(),
                        friendcode,
                    );
                }
                Some(Action::PickAvatar) => {
                    if let Some(path) = FileDialog::new()
                        .add_filter("image", &["png", "jpg", "jpeg", "gif", "webp"])
                        .pick_file()
                    {
                        // Load the image and open cropper
                        match load_image_for_cropping(&path) {
                            Ok(image) => {
                                app.identity_state.cropper_state = Some(crate::app::state::AvatarCropperState {
                                    image,
                                    pan: egui::vec2(0.0, 0.0),
                                    zoom: 1.0,
                                    source_path: path.display().to_string(),
                                });
                                app.identity_state.error = None;
                            }
                            Err(e) => {
                                app.identity_state.error = Some(format!("Failed to load image: {}", e));
                            }
                        }
                    }
                }
                None => {}
            }
        });
}

fn load_image_for_cropping(path: &std::path::Path) -> Result<egui::ColorImage, String> {
    let image_bytes = std::fs::read(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let image = image::load_from_memory(&image_bytes)
        .map_err(|e| format!("Failed to decode image: {}", e))?;

    let rgba = image.to_rgba8();
    let size = [rgba.width() as usize, rgba.height() as usize];
    let pixels = rgba.as_flat_samples();

    Ok(egui::ColorImage::from_rgba_unmultiplied(size, pixels.as_slice()))
}

pub(crate) fn render_avatar_cropper(app: &mut crate::app::GraphchanApp, ctx: &egui::Context) {
    let Some(cropper) = &mut app.identity_state.cropper_state else {
        return;
    };

    let mut should_close = false;
    let mut should_accept = false;

    egui::Window::new("Crop Avatar")
        .collapsible(false)
        .resizable(false)
        .fixed_size(egui::vec2(512.0 + 40.0, 512.0 + 100.0))
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.label("Drag to move, scroll to zoom");
                ui.add_space(10.0);

                // Draw the cropper canvas
                let (response, painter) = ui.allocate_painter(
                    egui::vec2(512.0, 512.0),
                    egui::Sense::click_and_drag(),
                );

                let rect = response.rect;
                let center = rect.center();

                // Handle zoom with scroll wheel
                if response.hovered() {
                    let scroll = ui.input(|i| i.raw_scroll_delta.y);
                    if scroll != 0.0 {
                        cropper.zoom *= 1.0 + scroll * 0.005;
                        cropper.zoom = cropper.zoom.clamp(0.1, 10.0);
                    }
                }

                // Handle drag
                if response.dragged() {
                    cropper.pan += response.drag_delta();
                }

                // Draw the image
                let img_size = egui::vec2(cropper.image.width() as f32, cropper.image.height() as f32);
                let scaled_size = img_size * cropper.zoom;
                let img_rect = egui::Rect::from_center_size(
                    center + cropper.pan,
                    scaled_size,
                );

                // Create and draw texture from the image
                let texture_id = format!("avatar_cropper_{}", cropper.source_path);
                let texture = ui.ctx().load_texture(
                    &texture_id,
                    cropper.image.clone(),
                    egui::TextureOptions::default(),
                );
                painter.image(
                    texture.id(),
                    img_rect,
                    egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                    egui::Color32::WHITE,
                );

                // Draw semi-transparent overlay outside the circle
                let radius = 256.0;
                let circle_center = center;

                // Draw darkened overlay with circle cutout
                // We'll use a mesh to draw the overlay with transparency
                painter.rect_filled(rect, egui::Rounding::ZERO, egui::Color32::from_black_alpha(100));

                // Draw circle outline
                painter.circle_stroke(
                    circle_center,
                    radius,
                    egui::Stroke::new(2.0, egui::Color32::WHITE),
                );

                // Draw inner transparent circle to show what will be kept
                // This is visual only - the overlay handles the transparency

                ui.add_space(20.0);

                ui.horizontal(|ui| {
                    ui.add_space(100.0);
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                    ui.add_space(20.0);
                    if ui.button("Accept").clicked() {
                        should_accept = true;
                    }
                });
            });
        });

    if should_close {
        app.identity_state.cropper_state = None;
    }

    if should_accept {
        // Crop and save
        if let Some(cropper) = app.identity_state.cropper_state.take() {
            match crop_to_circle(&cropper) {
                Ok(cropped_bytes) => {
                    // Save to a temporary file
                    let temp_path = std::env::temp_dir().join("graphchan_avatar_cropped.png");
                    if let Err(e) = std::fs::write(&temp_path, cropped_bytes) {
                        app.identity_state.error = Some(format!("Failed to save cropped avatar: {}", e));
                    } else {
                        app.identity_state.avatar_path = Some(temp_path.display().to_string());
                        // Auto-upload
                        crate::app::tasks::upload_avatar(
                            app.api.clone(),
                            app.tx.clone(),
                            temp_path.to_string_lossy().to_string(),
                        );
                        app.identity_state.uploading = true;
                    }
                }
                Err(e) => {
                    app.identity_state.error = Some(format!("Failed to crop avatar: {}", e));
                }
            }
        }
    }
}

fn crop_to_circle(cropper: &crate::app::state::AvatarCropperState) -> Result<Vec<u8>, String> {
    use image::{RgbaImage, Rgba};

    // Create a 512x512 output image
    let output_size = 512u32;
    let mut output = RgbaImage::new(output_size, output_size);

    let radius = 256.0f32;
    let center_x = 256.0f32;
    let center_y = 256.0f32;

    // Calculate the source image rectangle based on pan and zoom
    let img_width = cropper.image.width() as f32;
    let img_height = cropper.image.height() as f32;
    let scaled_width = img_width * cropper.zoom;
    let scaled_height = img_height * cropper.zoom;

    // Center of the 512x512 canvas
    let canvas_center_x = 256.0f32;
    let canvas_center_y = 256.0f32;

    // Top-left of the image in canvas space
    let img_left = canvas_center_x + cropper.pan.x - scaled_width / 2.0;
    let img_top = canvas_center_y + cropper.pan.y - scaled_height / 2.0;

    for y in 0..output_size {
        for x in 0..output_size {
            let px = x as f32;
            let py = y as f32;

            // Check if pixel is inside the circle
            let dx = px - center_x;
            let dy = py - center_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= radius {
                // Map canvas coordinates back to source image coordinates
                let src_x = (px - img_left) / cropper.zoom;
                let src_y = (py - img_top) / cropper.zoom;

                if src_x >= 0.0 && src_x < img_width && src_y >= 0.0 && src_y < img_height {
                    let src_x_int = src_x as usize;
                    let src_y_int = src_y as usize;
                    let idx = src_y_int * cropper.image.width() + src_x_int;

                    if idx < cropper.image.pixels.len() {
                        let color = cropper.image.pixels[idx];
                        output.put_pixel(x, y, Rgba([color.r(), color.g(), color.b(), color.a()]));
                    } else {
                        output.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                    }
                } else {
                    // Outside source image bounds - transparent
                    output.put_pixel(x, y, Rgba([0, 0, 0, 0]));
                }
            } else {
                // Outside circle - transparent
                output.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            }
        }
    }

    // Encode to PNG
    let mut png_bytes = Vec::new();
    let mut cursor = std::io::Cursor::new(&mut png_bytes);
    output.write_to(&mut cursor, image::ImageFormat::Png)
        .map_err(|e| format!("Failed to encode PNG: {}", e))?;

    Ok(png_bytes)
}
