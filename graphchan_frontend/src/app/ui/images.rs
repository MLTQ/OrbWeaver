use eframe::egui::{self, Color32, Context};

use crate::models::FileResponse;

use super::super::GraphchanApp;

impl GraphchanApp {
    pub(crate) fn render_file_attachment(
        &mut self,
        ui: &mut egui::Ui,
        file: &FileResponse,
        api_base: &str,
    ) {
        let download_url =
            super::super::resolve_download_url(api_base, file.download_url.as_deref(), &file.id);
        let name = file.original_name.as_deref().unwrap_or("attachment");
        let mime = file.mime.as_deref().unwrap_or("");

        let is_image = mime.starts_with("image/");

        if is_image && file.present {
            if let Some(tex) = self.image_textures.get(&file.id) {
                let mut response = ui.add(
                    egui::Image::from_texture(tex)
                        .max_width(150.0)
                        .sense(egui::Sense::click()),
                );
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    response = response.on_hover_text(name);
                }
                if response.clicked() {
                    self.image_viewers.insert(file.id.clone(), true);
                }
            } else if let Some(pending) = self.image_pending.remove(&file.id) {
                let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
                let tex = ui
                    .ctx()
                    .load_texture(&file.id, color, egui::TextureOptions::default());
                self.image_textures.insert(file.id.clone(), tex.clone());
                let mut response = ui.add(
                    egui::Image::from_texture(&tex)
                        .max_width(150.0)
                        .sense(egui::Sense::click()),
                );
                if response.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    response = response.on_hover_text(name);
                }
                if response.clicked() {
                    self.image_viewers.insert(file.id.clone(), true);
                }
            } else if let Some(err) = self.image_errors.get(&file.id) {
                ui.colored_label(Color32::LIGHT_RED, format!("Image failed: {err}"));
                ui.hyperlink_to(name, &download_url);
            } else {
                if !self.image_loading.contains(&file.id) {
                    self.spawn_download_image(&file.id, &download_url);
                }
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new());
                    ui.label(format!("Loading {}", name));
                });
            }
        } else {
            let label = if file.present {
                name.to_string()
            } else {
                format!("{} (remote)", name)
            };
            ui.hyperlink_to(label, download_url);
        }
    }

    pub(crate) fn render_image_viewers(&mut self, ctx: &Context) {
        let mut closed = Vec::new();
        for (id, open) in self.image_viewers.iter_mut() {
            egui::Window::new(format!("Image {}", id))
                .open(open)
                .resizable(true)
                .show(ctx, |ui| {
                    if let Some(tex) = self.image_textures.get(id) {
                        let avail = ui.available_size();
                        let mut size = tex.size_vec2();
                        let max_w = (avail.x - 16.0).max(200.0);
                        if size.x > max_w {
                            let scale = max_w / size.x;
                            size.x *= scale;
                            size.y *= scale;
                        }
                        ui.image(tex);
                    } else {
                        ui.label("Loading image...");
                    }
                });
            if !*open {
                closed.push(id.clone());
            }
        }
        for id in closed {
            self.image_viewers.remove(&id);
        }
    }
}
