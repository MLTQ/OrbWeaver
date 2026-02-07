use std::collections::VecDeque;
use std::path::PathBuf;

use eframe::egui;
use egui_commonmark::CommonMarkCache;
use egui_video::{AudioDevice, Player};

use super::{resolve_download_url, resolve_file_url, tasks, GraphchanApp};

// Maximum number of concurrent image downloads to prevent overwhelming the backend
const MAX_CONCURRENT_DOWNLOADS: usize = 4;

// Get the cache directory for videos, creating it if it doesn't exist
pub(super) fn get_video_cache_dir() -> Result<PathBuf, String> {
    let cache_dir = if let Some(home) = dirs::home_dir() {
        home.join(".graphchan").join("cache").join("videos")
    } else {
        PathBuf::from(".graphchan").join("cache").join("videos")
    };

    std::fs::create_dir_all(&cache_dir)
        .map_err(|e| format!("Failed to create cache directory: {}", e))?;

    Ok(cache_dir)
}

#[derive(Debug, Clone, PartialEq)]
pub enum FileType {
    Image,
    Video,
    Audio,
    Text,
    Pdf,
    Archive,
    Other,
}

impl FileType {
    pub fn from_mime(mime: &str) -> Self {
        match mime {
            m if m.starts_with("image/") => FileType::Image,
            m if m.starts_with("video/") => FileType::Video,
            m if m.starts_with("audio/") => FileType::Audio,
            m if m.starts_with("text/") => FileType::Text,
            "application/json" | "application/javascript" | "application/xml" => FileType::Text,
            "application/pdf" => FileType::Pdf,
            "application/zip" | "application/x-tar" | "application/gzip" | "application/x-7z-compressed" => FileType::Archive,
            _ => FileType::Other,
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            FileType::Image => "🖼",
            FileType::Video => "🎬",
            FileType::Audio => "🎵",
            FileType::Text => "📄",
            FileType::Pdf => "📕",
            FileType::Archive => "📦",
            FileType::Other => "📎",
        }
    }
}

#[derive(Debug, Clone)]
pub struct FileDownloadState {
    pub progress: f32,
    pub total_bytes: Option<usize>,
    pub downloaded_bytes: usize,
}

#[derive(Debug)]
pub struct FileViewerState {
    pub file_id: String,
    pub file_name: String,
    pub mime: String,
    pub content: FileViewerContent,
    pub markdown_cache: Option<CommonMarkCache>,
}

pub enum FileViewerContent {
    Loading,
    Text(String),
    Markdown(String),
    Video(Player),
    Pdf(Vec<u8>),
    Error(String),
}

impl std::fmt::Debug for FileViewerContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loading => write!(f, "Loading"),
            Self::Text(_) => write!(f, "Text(..)"),
            Self::Markdown(_) => write!(f, "Markdown(..)"),
            Self::Video(_) => write!(f, "Video(..)"),
            Self::Pdf(_) => write!(f, "Pdf(..)"),
            Self::Error(e) => write!(f, "Error({})", e),
        }
    }
}

fn format_file_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} bytes", bytes)
    }
}

impl GraphchanApp {
    pub(super) fn spawn_download_image(&mut self, file_id: &str, url: &str) {
        // Mark as loading
        self.image_loading.insert(file_id.to_string());

        // Add to queue
        self.download_queue.push_back((file_id.to_string(), url.to_string()));

        // Process queue
        self.process_download_queue();
    }

    pub fn spawn_load_image(&mut self, file_id: &str, url: &str) {
        self.spawn_download_image(file_id, url);
    }

    pub(super) fn process_download_queue(&mut self) {
        // Start downloads up to the limit
        while self.active_downloads < MAX_CONCURRENT_DOWNLOADS {
            if let Some((file_id, url)) = self.download_queue.pop_front() {
                self.active_downloads += 1;
                tasks::download_image(self.tx.clone(), file_id, url);
            } else {
                break;
            }
        }
    }

    pub(super) fn on_download_complete(&mut self) {
        // Decrement counter and process next item in queue
        if self.active_downloads > 0 {
            self.active_downloads -= 1;
        }
        self.process_download_queue();
    }

    pub fn render_file_attachment(
        &mut self,
        ui: &mut egui::Ui,
        file: &crate::models::FileResponse,
        base_url: &str,
    ) {
        let file_type = file.mime.as_ref()
            .map(|m| FileType::from_mime(m))
            .unwrap_or(FileType::Other);

        match file_type {
            FileType::Image => self.render_image_attachment(ui, file, base_url),
            _ => self.render_generic_file(ui, file, base_url, file_type),
        }
    }

    fn render_image_attachment(
        &mut self,
        ui: &mut egui::Ui,
        file: &crate::models::FileResponse,
        base_url: &str,
    ) {
        if let Some(texture) = self.image_textures.get(&file.id) {
            let size = texture.size_vec2();
            let max_width = 200.0;
            let scale = if size.x > max_width {
                max_width / size.x
            } else {
                1.0
            };
            let resp = ui.add(
                egui::Image::from_texture(texture)
                    .fit_to_exact_size(size * scale)
                    .sense(egui::Sense::click())
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.image_viewers.insert(file.id.clone(), true);
            }
        } else if let Some(pending) = self.image_pending.remove(&file.id) {
            let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
            let tex = ui.ctx().load_texture(&file.id, color, egui::TextureOptions::default());
            self.image_textures.insert(file.id.clone(), tex.clone());
            let size = tex.size_vec2();
            let max_width = 200.0;
            let scale = if size.x > max_width {
                max_width / size.x
            } else {
                1.0
            };
            let resp = ui.add(
                egui::Image::from_texture(&tex)
                    .fit_to_exact_size(size * scale)
                    .sense(egui::Sense::click())
            );
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            if resp.clicked() {
                self.image_viewers.insert(file.id.clone(), true);
            }
        } else if let Some(err) = self.image_errors.get(&file.id) {
            ui.colored_label(egui::Color32::RED, format!("Error: {}", err));
        } else {
            ui.spinner();
            if !self.image_loading.contains(&file.id) {
                let url = resolve_download_url(base_url, file.download_url.as_deref(), &file.id);
                self.spawn_download_image(&file.id, &url);
            }
        }
    }

    fn render_generic_file(
        &mut self,
        ui: &mut egui::Ui,
        file: &crate::models::FileResponse,
        base_url: &str,
        file_type: FileType,
    ) {
        ui.horizontal(|ui| {
            // File icon
            ui.label(file_type.icon());

            // File name and details
            let file_name = file.original_name.as_deref().unwrap_or("unnamed");
            let size_str = file.size_bytes
                .map(|s| format_file_size(s as u64))
                .unwrap_or_else(|| "unknown size".to_string());

            // Check if downloading
            if let Some(download_state) = self.file_downloads.get(&file.id) {
                ui.spinner();
                ui.label(format!("{} ({}%)", file_name, (download_state.progress * 100.0) as u32));
            } else {
                // Clickable file name
                let response = ui.add(egui::Label::new(
                    egui::RichText::new(file_name).underline().color(egui::Color32::LIGHT_BLUE)
                ).sense(egui::Sense::click()));

                if response.clicked() {
                    // Open file viewer/downloader
                    self.open_file_viewer(&file.id, file_name, file.mime.as_deref().unwrap_or("application/octet-stream"), base_url);
                }

                // Show context menu on right-click
                response.context_menu(|ui| {
                    if ui.button("💾 Save as...").clicked() {
                        self.save_file_as(&file.id, file_name, base_url);
                        ui.close_menu();
                    }
                    if ui.button("📋 Copy link").clicked() {
                        let url = resolve_file_url(base_url, &file.id);
                        ui.output_mut(|o| o.copied_text = url);
                        ui.close_menu();
                    }
                });

                ui.label(format!("({})", size_str));
            }
        });
    }

    pub(crate) fn open_file_viewer(&mut self, file_id: &str, file_name: &str, mime: &str, base_url: &str) {
        let file_type = FileType::from_mime(mime);

        // Create viewer state
        let viewer = FileViewerState {
            file_id: file_id.to_string(),
            file_name: file_name.to_string(),
            mime: mime.to_string(),
            content: FileViewerContent::Loading,
            markdown_cache: None,
        };

        self.file_viewers.insert(file_id.to_string(), viewer);

        // Start download based on file type
        match file_type {
            FileType::Text => self.download_text_file(file_id, base_url),
            FileType::Video | FileType::Audio => self.download_media_file(file_id, base_url),
            FileType::Pdf => self.download_pdf_file(file_id, base_url),
            _ => self.download_generic_file(file_id, file_name, base_url),
        }
    }

    fn save_file_as(&mut self, file_id: &str, suggested_name: &str, base_url: &str) {
        let url = resolve_file_url(base_url, file_id);
        let file_id = file_id.to_string();
        let suggested_name = suggested_name.to_string();

        tasks::save_file_as(self.tx.clone(), file_id, url, suggested_name);
    }

    fn download_text_file(&mut self, file_id: &str, base_url: &str) {
        let url = resolve_file_url(base_url, file_id);
        tasks::download_text_file(self.tx.clone(), file_id.to_string(), url);
    }

    fn download_media_file(&mut self, file_id: &str, base_url: &str) {
        // Check if video is already cached
        if let Ok(cache_dir) = get_video_cache_dir() {
            let cache_path = cache_dir.join(format!("{}.mp4", file_id));
            if cache_path.exists() {
                log::info!("Loading cached video: {}", cache_path.display());
                // Load from cache instead of downloading
                if let Some(ctx) = &self.ctx {
                    let player_result = if let Some(audio_device) = &mut self.audio_device {
                        Player::new(ctx, &cache_path.to_string_lossy().to_string())
                            .and_then(|mut player| {
                                // Set initial volume
                                player.options.set_audio_volume(self.video_volume);
                                player.with_audio(audio_device)
                            })
                    } else {
                        log::warn!("No audio device available, creating player without audio");
                        Player::new(ctx, &cache_path.to_string_lossy().to_string())
                    };

                    match player_result {
                        Ok(player) => {
                            if let Some(viewer) = self.file_viewers.get_mut(file_id) {
                                viewer.content = FileViewerContent::Video(player);
                            }
                            return;
                        }
                        Err(err) => {
                            log::error!("Failed to load cached video: {}", err);
                            // Fall through to download
                        }
                    }
                }
            }
        }

        // Not cached or failed to load from cache - download it
        let url = resolve_file_url(base_url, file_id);
        tasks::download_media_file(self.tx.clone(), file_id.to_string(), url);
    }

    fn download_pdf_file(&mut self, file_id: &str, base_url: &str) {
        let url = resolve_file_url(base_url, file_id);
        tasks::download_pdf_file(self.tx.clone(), file_id.to_string(), url);
    }

    fn download_generic_file(&mut self, file_id: &str, file_name: &str, base_url: &str) {
        let url = resolve_file_url(base_url, file_id);
        tasks::save_file_as(self.tx.clone(), file_id.to_string(), url, file_name.to_string());
    }

    pub(super) fn render_file_viewers(&mut self, ctx: &egui::Context) {
        let mut to_close = Vec::new();
        let mut save_actions = Vec::new();

        // Collect file IDs to iterate over
        let viewer_ids: Vec<String> = self.file_viewers.keys().cloned().collect();

        for file_id in viewer_ids {
            if let Some(viewer) = self.file_viewers.get_mut(&file_id) {
                let mut is_open = true;
                let file_name = viewer.file_name.clone();

                egui::Window::new(&file_name)
                    .id(egui::Id::new(format!("file_viewer_{}", file_id)))
                    .open(&mut is_open)
                    .default_size([800.0, 600.0])
                    .show(ctx, |ui| {
                        match &mut viewer.content {
                            FileViewerContent::Loading => {
                                ui.vertical_centered(|ui| {
                                    ui.spinner();
                                    ui.label("Loading file...");
                                });
                            }
                            FileViewerContent::Text(text) => {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    ui.add(
                                        egui::TextEdit::multiline(&mut text.as_str())
                                            .desired_width(f32::INFINITY)
                                            .code_editor()
                                    );
                                });
                            }
                            FileViewerContent::Markdown(text) => {
                                egui::ScrollArea::vertical().show(ui, |ui| {
                                    if let Some(cache) = &mut viewer.markdown_cache {
                                        egui_commonmark::CommonMarkViewer::new().show(ui, cache, text);
                                    } else {
                                        // Fallback to plain text if cache not initialized
                                        ui.label(text.as_str());
                                    }
                                });
                            }
                            FileViewerContent::Video(player) => {
                                ui.vertical_centered(|ui| {
                                    // Render the video player with built-in controls
                                    // egui-video Player includes play/pause/seek controls
                                    player.ui(ui, egui::vec2(640.0, 480.0));

                                    ui.add_space(10.0);

                                    // Volume control (collapsible)
                                    ui.collapsing("🔊 Volume", |ui| {
                                        ui.horizontal(|ui| {
                                            let mut volume = self.video_volume;
                                            ui.label("Volume:");
                                            if ui.add(egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false)).changed() {
                                                self.video_volume = volume;
                                                // Update player volume
                                                player.options.set_audio_volume(volume);
                                            }
                                            ui.label(format!("{}%", (volume * 100.0) as i32));
                                        });
                                    });

                                    ui.add_space(5.0);

                                    // Save button
                                    if ui.button("💾 Save video").clicked() {
                                        save_actions.push((file_id.clone(), file_name.clone()));
                                    }
                                });
                            }
                            FileViewerContent::Pdf(_bytes) => {
                                // Simplified PDF viewer for now
                                // TODO: Add full PDF page rendering with pdfium-render
                                ui.vertical_centered(|ui| {
                                    ui.heading("PDF Viewer");
                                    ui.add_space(10.0);
                                    ui.label("PDF page rendering will be added in a future update.");
                                    ui.label("For now, you can save the PDF and open it externally.");
                                    ui.add_space(10.0);

                                    // Save button
                                    if ui.button("💾 Save PDF").clicked() {
                                        save_actions.push((file_id.clone(), file_name.clone()));
                                    }
                                });
                            }
                            FileViewerContent::Error(err) => {
                                ui.vertical_centered(|ui| {
                                    ui.colored_label(egui::Color32::RED, "Error loading file:");
                                    ui.label(err.as_str());
                                });
                            }
                        }
                    });

                if !is_open {
                    to_close.push(file_id);
                }
            }
        }

        // Process save actions after releasing borrow
        for (file_id, file_name) in save_actions {
            let base_url = self.base_url_input.clone();
            self.save_file_as(&file_id, &file_name, &base_url);
        }

        // Close viewers
        for file_id in to_close {
            self.file_viewers.remove(&file_id);
        }
    }
}
