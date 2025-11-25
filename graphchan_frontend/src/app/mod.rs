use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use chrono::{DateTime, Utc};
use eframe::egui::{self, Context, TextureHandle};
use egui_video::{AudioDevice, Player};
use log::error;

use crate::api::ApiClient;
// ImageLoader removed; using custom async pipeline
use crate::models::{CreatePostInput, CreateThreadInput, PeerView, ThreadSummary};

mod messages;
mod state;
mod tasks;
mod ui;

use messages::AppMessage;
use state::{
    CreateThreadState, ImporterState, LoadedImage, ThreadDisplayMode, ThreadState, ViewState,
};

// Maximum number of concurrent image downloads to prevent overwhelming the backend
const MAX_CONCURRENT_DOWNLOADS: usize = 4;

// Get the cache directory for videos, creating it if it doesn't exist
fn get_video_cache_dir() -> Result<PathBuf, String> {
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

pub struct GraphchanApp {
    api: ApiClient,
    tx: Sender<AppMessage>,
    rx: Receiver<AppMessage>,
    threads: Vec<ThreadSummary>,
    threads_loading: bool,
    threads_error: Option<String>,
    view: ViewState,
    show_create_thread: bool,
    create_thread: CreateThreadState,
    base_url_input: String,
    info_banner: Option<String>,
    importer: ImporterState,
    pending_thread_focus: Option<String>,
    image_textures: HashMap<String, TextureHandle>,
    image_loading: HashSet<String>,
    image_pending: HashMap<String, LoadedImage>,
    image_errors: HashMap<String, String>,
    image_viewers: HashMap<String, bool>,
    // Download queue for rate limiting
    download_queue: VecDeque<(String, String)>, // (file_id, url)
    active_downloads: usize,
    peers: HashMap<String, PeerView>,
    show_identity: bool,
    identity_state: state::IdentityState,
    file_downloads: HashMap<String, FileDownloadState>,
    file_viewers: HashMap<String, FileViewerState>,
    ctx: Option<egui::Context>,
    audio_device: Option<AudioDevice>,
    video_volume: f32, // 0.0 to 1.0
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
}

pub enum FileViewerContent {
    Loading,
    Text(String),
    Video(Player),
    Pdf(Vec<u8>),
    Error(String),
}

impl std::fmt::Debug for FileViewerContent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Loading => write!(f, "Loading"),
            Self::Text(_) => write!(f, "Text(..)"),
            Self::Video(_) => write!(f, "Video(..)"),
            Self::Pdf(_) => write!(f, "Pdf(..)"),
            Self::Error(e) => write!(f, "Error({})", e),
        }
    }
}

pub(crate) fn resolve_download_url(
    base_url: &str,
    download_url: Option<&str>,
    file_id: &str,
) -> String {
    if let Some(url) = download_url {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else if url.starts_with('/') {
            format!("{base_url}{url}")
        } else {
            format!("{base_url}/{url}")
        }
    } else {
        format!("{base_url}/files/{file_id}")
    }
}

pub(crate) fn resolve_blob_url(base_url: &str, blob_id: &str) -> String {
    format!("{base_url}/blobs/{blob_id}")
}

pub(crate) fn resolve_file_url(base_url: &str, file_id: &str) -> String {
    format!("{base_url}/files/{file_id}")
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
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let default_url = std::env::var("GRAPHCHAN_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        let api = ApiClient::new(default_url.clone()).unwrap_or_else(|err| {
            error!("failed to initialise API client: {err}");
            ApiClient::new("http://127.0.0.1:8080").expect("fallback API client")
        });
        let (tx, rx) = mpsc::channel();

        // Initialize SDL2 audio for video playback
        let audio_device = match sdl2::init() {
            Ok(sdl_context) => {
                match sdl_context.audio() {
                    Ok(audio_subsystem) => {
                        match AudioDevice::from_subsystem(&audio_subsystem) {
                            Ok(device) => {
                                log::info!("SDL2 audio initialized successfully");
                                Some(device)
                            }
                            Err(e) => {
                                log::error!("Failed to create audio device: {}", e);
                                None
                            }
                        }
                    }
                    Err(e) => {
                        log::error!("Failed to initialize SDL2 audio subsystem: {}", e);
                        None
                    }
                }
            }
            Err(e) => {
                log::error!("Failed to initialize SDL2: {}", e);
                None
            }
        };

        let mut app = Self {
            api,
            tx,
            rx,
            threads: Vec::new(),
            threads_loading: false,
            threads_error: None,
            view: ViewState::Catalog,
            show_create_thread: false,
            create_thread: CreateThreadState::default(),
            base_url_input: default_url,
            info_banner: None,
            importer: ImporterState::default(),
            pending_thread_focus: None,
            image_textures: HashMap::new(),
            image_loading: HashSet::new(),
            image_pending: HashMap::new(),
            image_errors: HashMap::new(),
            image_viewers: HashMap::new(),
            download_queue: VecDeque::new(),
            active_downloads: 0,
            peers: HashMap::new(),
            show_identity: false,
            identity_state: state::IdentityState::default(),
            file_downloads: HashMap::new(),
            file_viewers: HashMap::new(),
            ctx: None,
            audio_device,
            video_volume: 0.8, // Default to 80% volume
        };
        app.spawn_load_threads();
        app.spawn_load_peers();
        app
    }

    fn spawn_load_threads(&mut self) {
        if self.threads_loading {
            return;
        }
        self.threads_loading = true;
        self.threads_error = None;
        tasks::load_threads(self.api.clone(), self.tx.clone());
    }

    fn spawn_load_thread(&mut self, thread_id: &str) {
        tasks::load_thread(self.api.clone(), self.tx.clone(), thread_id.to_string());
    }

    fn spawn_create_thread(&mut self) {
        let title = self.create_thread.title.trim().to_string();
        if title.is_empty() {
            self.create_thread.error = Some("Title cannot be empty".into());
            return;
        }
        let body = self.create_thread.body.trim().to_string();
        let mut payload = CreateThreadInput::default();
        payload.title = title;
        if !body.is_empty() {
            payload.body = Some(body);
        }
        self.create_thread.submitting = true;
        self.create_thread.error = None;
        tasks::create_thread(self.api.clone(), self.tx.clone(), payload, self.create_thread.files.clone());
    }

    fn spawn_create_post(&mut self, thread_state: &mut ThreadState) {
        let body = thread_state.new_post_body.trim().to_string();
        if body.is_empty() {
            thread_state.new_post_error = Some("Post body cannot be empty".into());
            return;
        }
        let thread_id = thread_state.summary.id.clone();
        let mut payload = CreatePostInput::default();
        payload.thread_id = thread_id.clone();
        payload.body = body;
        payload.parent_post_ids = thread_state.reply_to.clone();
        let attachments = thread_state.draft_attachments.clone();
        thread_state.new_post_sending = true;
        thread_state.new_post_error = None;
        tasks::create_post(self.api.clone(), self.tx.clone(), thread_id, payload, attachments);
    }

    fn spawn_import_fourchan(&mut self) {
        let url = self.importer.url.trim().to_string();
        if url.is_empty() {
            self.importer.error = Some("Paste a full 4chan thread URL".into());
            return;
        }
        self.importer.importing = true;
        self.importer.error = None;
        tasks::import_fourchan(self.api.clone(), self.tx.clone(), url);
    }

    fn spawn_load_identity(&mut self) {
        tasks::load_identity(self.api.clone(), self.tx.clone());
    }

    fn spawn_load_peers(&mut self) {
        tasks::load_peers(self.api.clone(), self.tx.clone());
    }



    fn process_messages(&mut self) {
        messages::process_messages(self);
    }

    fn spawn_download_image(&mut self, file_id: &str, url: &str) {
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
    
    fn process_download_queue(&mut self) {
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
    
    fn on_download_complete(&mut self) {
        // Decrement counter and process next item in queue
        if self.active_downloads > 0 {
            self.active_downloads -= 1;
        }
        self.process_download_queue();
    }

    fn set_reply_target(state: &mut ThreadState, post_id: &str) {
        state.reply_to.clear();
        state.reply_to.push(post_id.to_string());
    }

    fn quote_post(state: &mut ThreadState, post_id: &str) {
        Self::set_reply_target(state, post_id);
        let quote_line = format!(">>{post_id}\n");
        if !state.new_post_body.starts_with(&quote_line) {
            state.new_post_body = format!("{quote_line}{}", state.new_post_body);
        }
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
            ui.add(egui::Image::from_texture(texture).fit_to_exact_size(size * scale));
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
            ui.add(egui::Image::from_texture(&tex).fit_to_exact_size(size * scale));
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

    fn open_file_viewer(&mut self, file_id: &str, file_name: &str, mime: &str, base_url: &str) {
        let file_type = FileType::from_mime(mime);

        // Create viewer state
        let viewer = FileViewerState {
            file_id: file_id.to_string(),
            file_name: file_name.to_string(),
            mime: mime.to_string(),
            content: FileViewerContent::Loading,
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

    fn render_file_viewers(&mut self, ctx: &egui::Context) {
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
                            FileViewerContent::Pdf(bytes) => {
                                ui.vertical_centered(|ui| {
                                    ui.label(format!("PDF file ({} bytes)", bytes.len()));
                                    ui.label("PDF viewing not yet implemented");
                                    ui.label("Use 'Save as...' to download and open externally");

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

impl eframe::App for GraphchanApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Store context for video player initialization
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }

        self.process_messages();

        egui::TopBottomPanel::top("top_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("API Base URL");
                ui.text_edit_singleline(&mut self.base_url_input);
                if ui.button("Apply").clicked() {
                    match self.api.set_base_url(self.base_url_input.clone()) {
                        Ok(()) => {
                            self.info_banner = Some("API URL updated".into());
                            self.spawn_load_threads();
                        }
                        Err(err) => {
                            self.info_banner = Some(format!("Failed to update URL: {err}"));
                        }
                    }
                }
                if ui.button("Refresh").clicked() {
                    self.spawn_load_threads();
                }
                if ui.button("New Thread").clicked() {
                    self.show_create_thread = true;
                }
                if ui.button("Import 4chan…").clicked() {
                    self.importer.open = true;
                }
                if ui.selectable_label(self.show_identity, "Identity").clicked() {
                    self.show_identity = !self.show_identity;
                }
            });

            if let Some(message) = self.info_banner.clone() {
                let mut dismiss = false;
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(message.as_str());
                            if ui.button("Dismiss").clicked() {
                                dismiss = true;
                            }
                        });
                    });
                if dismiss {
                    self.info_banner = None;
                }
            }
        });

        let mut go_back = false;

        // We need to extract mutable state to avoid double-borrowing self in the CentralPanel closure
        let view_type = match &self.view {
            ViewState::Catalog => "catalog",
            ViewState::Thread(_) => "thread",
        };

        if view_type == "catalog" {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.render_catalog(ui);
            });
        } else if view_type == "thread" {
            // Extract thread state temporarily
            let mut temp_state = if let ViewState::Thread(state) = &mut self.view {
                std::mem::replace(
                    state,
                    ThreadState {
                        summary: ThreadSummary {
                            id: String::new(),
                            title: String::new(),
                            creator_peer_id: None,
                            created_at: String::new(),
                            pinned: false,
                        },
                        details: None,
                        is_loading: false,
                        error: None,
                        new_post_body: String::new(),
                        new_post_error: None,
                        new_post_sending: false,
                        attachments: HashMap::new(),
                        attachments_loading: HashSet::new(),
                        attachments_errors: HashMap::new(),
                        display_mode: ThreadDisplayMode::List,
                        last_layout_mode: None,
                        graph_nodes: HashMap::new(),
                        chronological_nodes: HashMap::new(),
                        sim_start_time: None,
                        selected_post: None,
                        graph_zoom: 1.0,
                        graph_offset: egui::vec2(0.0, 0.0),
                        graph_dragging: false,
                        reply_to: Vec::new(),
                        time_bin_seconds: 60,
                        locked_hover_post: None,
                        repulsion_force: 500.0,
                        sim_paused: false,
                        draft_attachments: Vec::new(),
                    },
                )
            } else {
                unreachable!()
            };

            let mut thread_action = crate::app::ui::thread::ThreadAction::None;
            egui::CentralPanel::default().show(ctx, |ui| {
                thread_action = self.render_thread(ui, &mut temp_state);
            });

            // Restore state
            if let ViewState::Thread(state) = &mut self.view {
                *state = temp_state;
            }

            let mut go_back = false;
            match thread_action {
                crate::app::ui::thread::ThreadAction::GoBack => go_back = true,
                crate::app::ui::thread::ThreadAction::OpenThread(id) => {
                    self.spawn_load_thread(&id);
                }
                _ => {}
            }

            if go_back {
                self.view = ViewState::Catalog;
                self.spawn_load_threads();
            }
        }



        self.render_create_thread_dialog(ctx);
        self.render_import_dialog(ctx);
        ui::drawer::render_identity_drawer(self, ctx);
        ui::drawer::render_avatar_cropper(self, ctx);
        self.render_file_viewers(ctx);
    }
}

fn format_timestamp(ts: &str) -> String {
    DateTime::parse_from_rfc3339(ts)
        .map(|dt| {
            dt.with_timezone(&Utc)
                .format("%Y-%m-%d %H:%M UTC")
                .to_string()
        })
        .unwrap_or_else(|_| ts.to_string())
}
