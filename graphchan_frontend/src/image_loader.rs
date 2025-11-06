use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use eframe::egui::{self, ColorImage, Context, TextureHandle};
use image::DynamicImage;

lazy_static::lazy_static! {
    /// Global cache of loaded images (DynamicImage format, not yet textures)
    static ref IMAGE_STATES: Arc<Mutex<HashMap<String, ImageLoadState>>> = 
        Arc::new(Mutex::new(HashMap::new()));
}

enum ImageLoadState {
    Loading,              // Started loading, not done yet
    Loaded(DynamicImage), // Successfully loaded
    Failed(String),       // Failed with error
}

/// Simple image loader that:
/// 1. Downloads images to local cache (one HTTP request per image)
/// 2. Loads from cache using filesystem (fast, no HTTP overhead)
/// 3. Creates textures on-demand in UI thread
pub struct ImageLoader {
    cache_dir: PathBuf,
    textures: HashMap<String, TextureHandle>,
}

impl ImageLoader {
    pub fn new() -> Self {
        let cache_dir = PathBuf::from("cache/images");
        fs::create_dir_all(&cache_dir).ok();
        
        Self {
            cache_dir,
            textures: HashMap::new(),
        }
    }
    
    /// Ensure image is in local cache, download if needed
    /// Uses original_name to preserve file extension

    
    /// Render an image, loading from cache if needed
    /// Returns true if image was displayed, false if loading
    pub fn render_image(
        &mut self,
        ui: &mut egui::Ui,
        file_id: &str,
        file_name: &str,
        download_url: &str,
        max_width: f32,
    ) -> bool {
        // Do we already have a texture?
        if let Some(texture) = self.textures.get(file_id) {
            let response = ui.add(
                egui::Image::new(texture)
                    .max_width(max_width)
                    .shrink_to_fit()
                    .sense(egui::Sense::click())
            );
            
            if response.clicked() {
                // Open full size in browser
                let _ = open::that(download_url);
            }
            
            if response.hovered() {
                response.on_hover_text(format!("Click to open: {}", file_name));
            }
            
            return true;
        }
        
        // Check global loading state
        let ctx = ui.ctx().clone();
        let mut states = IMAGE_STATES.lock().unwrap();
        
        match states.get(file_id) {
            None => {
                // Start async download + decode without blocking UI
                states.insert(file_id.to_string(), ImageLoadState::Loading);
                drop(states);

                let file_id_owned = file_id.to_string();
                let file_name_owned = file_name.to_string();
                let download_url_owned = download_url.to_string();
                let cache_dir = self.cache_dir.clone();
                let states_clone = Arc::clone(&IMAGE_STATES);
                thread::spawn(move || {
                    // Build cache path
                    let cache_filename = if file_name_owned.contains('.') {
                        format!("{}_{}", file_id_owned, file_name_owned)
                    } else {
                        file_id_owned.clone()
                    };
                    let cache_path = cache_dir.join(&cache_filename);
                    // Download if missing
                    if !cache_path.exists() {
                        log::info!("Downloading image: {} -> {}", download_url_owned, cache_filename);
                        if let Ok(resp) = reqwest::blocking::get(&download_url_owned) {
                            if let Ok(bytes) = resp.bytes() {
                                if fs::write(&cache_path, &bytes).is_err() {
                                    log::warn!("Failed to write cache file for {}", file_id_owned);
                                }
                            }
                        }
                    }
                    // Read & decode
                    match fs::read(&cache_path) {
                        Ok(bytes) => match image::load_from_memory(&bytes) {
                            Ok(img) => {
                                states_clone.lock().unwrap().insert(file_id_owned.clone(), ImageLoadState::Loaded(img));
                            }
                            Err(e) => {
                                log::error!("Decode failed for {}: {}", file_id_owned, e);
                                states_clone.lock().unwrap().insert(file_id_owned.clone(), ImageLoadState::Failed(e.to_string()));
                            }
                        },
                        Err(e) => {
                            log::error!("Read failed for {}: {}", file_id_owned, e);
                            states_clone.lock().unwrap().insert(file_id_owned.clone(), ImageLoadState::Failed(e.to_string()));
                        }
                    }
                    ctx.request_repaint();
                });

                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Loading {}...", file_name));
                });
                false
            }
            Some(ImageLoadState::Loading) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(format!("Loading {}...", file_name));
                });
                false
            }
            Some(ImageLoadState::Failed(err)) => {
                ui.colored_label(egui::Color32::LIGHT_RED, format!("Failed: {}", err));
                ui.hyperlink_to("Download", download_url);
                false
            }
            Some(ImageLoadState::Loaded(img)) => {
                // Create texture now (must be done in UI thread)
                let texture = Self::dynamic_image_to_texture(&ctx, img, file_id);
                self.textures.insert(file_id.to_string(), texture.clone());
                states.remove(file_id); // Clean up
                drop(states);
                
                // Display it
                let response = ui.add(
                    egui::Image::new(&texture)
                        .max_width(max_width)
                        .shrink_to_fit()
                        .sense(egui::Sense::click())
                );
                
                if response.clicked() {
                    let _ = open::that(download_url);
                }
                
                if response.hovered() {
                    response.on_hover_text(format!("Click to open: {}", file_name));
                }
                
                true
            }
        }
    }
    
    fn dynamic_image_to_texture(ctx: &Context, img: &DynamicImage, name: &str) -> TextureHandle {
        let rgba = img.to_rgba8();
        let size = [img.width() as usize, img.height() as usize];
        let pixels = rgba.as_flat_samples();
        
        let color_image = ColorImage::from_rgba_unmultiplied(size, pixels.as_slice());
        
        ctx.load_texture(name, color_image, egui::TextureOptions::default())
    }
}
