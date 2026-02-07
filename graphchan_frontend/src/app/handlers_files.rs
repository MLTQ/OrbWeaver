use log::error;

use super::file_viewer::{get_video_cache_dir, FileViewerContent};
use super::GraphchanApp;
use super::state::{LoadedImage, ViewState};

impl GraphchanApp {
    pub(super) fn handle_image_loaded(&mut self, file_id: String, result: Result<LoadedImage, String>) {
        self.image_loading.remove(&file_id);
        match result {
            Ok(img) => {
                self.image_pending.insert(file_id, img);
            }
            Err(e) => {
                error!("Failed to load image {}: {}", file_id, e);
                self.image_errors.insert(file_id, e);
            }
        }
        // Download completed, process next item in queue
        self.on_download_complete();
    }

    pub(super) fn handle_text_file_loaded(&mut self, file_id: String, result: Result<String, String>) {
        if let Some(viewer) = self.file_viewers.get_mut(&file_id) {
            viewer.content = match result {
                Ok(text) => {
                    // Detect if it's markdown based on file extension or content
                    let is_markdown = viewer.file_name.ends_with(".md")
                        || viewer.file_name.ends_with(".markdown")
                        || viewer.mime.contains("markdown");

                    if is_markdown {
                        // Initialize markdown cache
                        viewer.markdown_cache = Some(egui_commonmark::CommonMarkCache::default());
                        FileViewerContent::Markdown(text)
                    } else {
                        FileViewerContent::Text(text)
                    }
                }
                Err(err) => FileViewerContent::Error(err),
            };
        }
    }

    pub(super) fn handle_media_file_loaded(&mut self, file_id: String, result: Result<Vec<u8>, String>) {
        if let Some(viewer) = self.file_viewers.get_mut(&file_id) {
            viewer.content = match result {
                Ok(bytes) => {
                    // Save to persistent cache directory
                    match get_video_cache_dir() {
                        Ok(cache_dir) => {
                            let cache_path = cache_dir.join(format!("{}.mp4", file_id));
                            match std::fs::write(&cache_path, bytes) {
                                Ok(()) => {
                                    log::info!("Cached video to: {}", cache_path.display());

                                    // Create player with audio support
                                    let player_result = if let Some(audio_device) = &mut self.audio_device {
                                        egui_video::Player::new(self.ctx.as_ref().unwrap(), &cache_path.to_string_lossy().to_string())
                                            .and_then(|mut player| {
                                                // Set initial volume
                                                player.options.set_audio_volume(self.video_volume);
                                                player.with_audio(audio_device)
                                            })
                                    } else {
                                        log::warn!("No audio device available, creating player without audio");
                                        egui_video::Player::new(self.ctx.as_ref().unwrap(), &cache_path.to_string_lossy().to_string())
                                    };

                                    match player_result {
                                        Ok(player) => FileViewerContent::Video(player),
                                        Err(err) => FileViewerContent::Error(format!("Failed to load video: {}", err)),
                                    }
                                }
                                Err(err) => FileViewerContent::Error(format!("Failed to cache video: {}", err)),
                            }
                        }
                        Err(err) => FileViewerContent::Error(err),
                    }
                }
                Err(err) => FileViewerContent::Error(err),
            };
        }
    }

    pub(super) fn handle_pdf_file_loaded(&mut self, file_id: String, result: Result<Vec<u8>, String>) {
        if let Some(viewer) = self.file_viewers.get_mut(&file_id) {
            viewer.content = match result {
                Ok(bytes) => FileViewerContent::Pdf(bytes),
                Err(err) => FileViewerContent::Error(err),
            };
        }
    }

    pub(super) fn handle_file_saved(&mut self, result: Result<(), String>) {
        match result {
            Ok(()) => {
                self.info_banner = Some("File saved successfully".to_string());
            }
            Err(err) => {
                self.info_banner = Some(format!("Failed to save file: {}", err));
            }
        }
    }
}
