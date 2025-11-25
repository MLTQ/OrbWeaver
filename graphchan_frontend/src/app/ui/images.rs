use eframe::egui::{self, Context};

use super::super::GraphchanApp;

impl GraphchanApp {
    pub(crate) fn render_image_viewers(&mut self, ctx: &Context) {
        let mut closed = Vec::new();
        for (id, open) in self.image_viewers.iter_mut() {
            egui::Window::new(format!("Image {}", id))
                .open(open)
                .default_size([512.0, 512.0])
                .resizable(true)
                .show(ctx, |ui| {
                    if let Some(tex) = self.image_textures.get(id) {
                        ui.add(
                            egui::Image::from_texture(tex)
                                .shrink_to_fit()
                        );
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
