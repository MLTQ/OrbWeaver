use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use chrono::{DateTime, Utc};
use eframe::egui::{self, Context, TextureHandle};
use egui_commonmark::CommonMarkCache;
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
    BlockingState, ConversationState, CreateThreadState, DmState, ImporterState, LoadedImage,
    RedditImporterState, ThreadDisplayMode, ThreadState, ViewState,
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
    reddit_importer: RedditImporterState,
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
    show_ignored_threads: bool, // Toggle to show/hide ignored threads in peer catalog
    dm_state: DmState,
    blocking_state: BlockingState,
    auto_refresh_enabled: bool, // Toggle for auto-refresh
    is_refreshing: bool, // Track if currently refreshing
    last_refresh_time: Option<std::time::Instant>, // Track animation timing
    search_query_input: String,
    search_focused: bool,
    // Recent posts feed
    recent_posts: Vec<crate::models::RecentPostView>,
    recent_posts_loading: bool,
    recent_posts_error: Option<String>,
    last_recent_posts_refresh: Option<std::time::Instant>,
    recent_posts_poll_interval: u64, // In seconds
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
            reddit_importer: RedditImporterState::default(),
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
            show_ignored_threads: false, // Default to hiding ignored threads
            dm_state: DmState::default(),
            blocking_state: BlockingState::default(),
            auto_refresh_enabled: true, // Auto-refresh on by default
            is_refreshing: false,
            last_refresh_time: None,
            search_query_input: String::new(),
            search_focused: false,
            // Recent posts feed
            recent_posts: Vec::new(),
            recent_posts_loading: false,
            recent_posts_error: None,
            last_recent_posts_refresh: None,
            recent_posts_poll_interval: 5, // Poll every 5 seconds by default
        };
        app.spawn_load_threads();
        app.spawn_load_recent_posts();
        app.spawn_load_peers();
        app.spawn_load_conversations();
        app.spawn_load_blocked_peers();
        app.spawn_load_blocklists();
        app.spawn_load_ip_blocks();
        app.spawn_load_ip_block_stats();
        app
    }

    fn spawn_load_threads(&mut self) {
        if self.threads_loading {
            return;
        }
        self.threads_loading = true;
        self.threads_error = None;
        self.is_refreshing = true;
        self.last_refresh_time = Some(std::time::Instant::now());
        tasks::load_threads(self.api.clone(), self.tx.clone());
    }

    fn spawn_load_thread(&mut self, thread_id: &str) {
        tasks::load_thread(self.api.clone(), self.tx.clone(), thread_id.to_string(), false);
    }

    fn spawn_refresh_thread(&mut self, thread_id: &str) {
        tasks::load_thread(self.api.clone(), self.tx.clone(), thread_id.to_string(), true);
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
        payload.rebroadcast = thread_state.is_hosting; // Use the Host/Leech toggle state
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

    fn spawn_load_recent_posts(&mut self) {
        if self.recent_posts_loading {
            return;
        }
        self.recent_posts_loading = true;
        self.recent_posts_error = None;
        self.last_recent_posts_refresh = Some(std::time::Instant::now());
        tasks::load_recent_posts(self.api.clone(), self.tx.clone());
    }

    fn spawn_load_conversations(&mut self) {
        if self.dm_state.conversations_loading {
            return;
        }
        self.dm_state.conversations_loading = true;
        self.dm_state.conversations_error = None;
        tasks::load_conversations(self.api.clone(), self.tx.clone());
    }

    fn spawn_load_messages(&mut self, state: &mut ConversationState) {
        if state.messages_loading {
            return;
        }
        state.messages_loading = true;
        state.messages_error = None;
        tasks::load_messages(self.api.clone(), self.tx.clone(), state.peer_id.clone());
    }

    fn spawn_send_dm(&mut self, state: &mut ConversationState) {
        let body = state.new_message_body.trim().to_string();
        if body.is_empty() {
            state.send_error = Some("Message cannot be empty".into());
            return;
        }
        state.sending = true;
        state.send_error = None;
        tasks::send_dm(
            self.api.clone(),
            self.tx.clone(),
            state.peer_id.clone(),
            body,
        );
    }

    fn spawn_load_blocked_peers(&mut self) {
        if self.blocking_state.blocked_peers_loading {
            return;
        }
        self.blocking_state.blocked_peers_loading = true;
        self.blocking_state.blocked_peers_error = None;
        tasks::load_blocked_peers(self.api.clone(), self.tx.clone());
    }

    fn spawn_block_peer(&mut self, state: &mut state::BlockingState) {
        let peer_id = state.new_block_peer_id.trim().to_string();
        if peer_id.is_empty() {
            state.block_error = Some("Peer ID cannot be empty".into());
            return;
        }
        let reason = if state.new_block_reason.trim().is_empty() {
            None
        } else {
            Some(state.new_block_reason.trim().to_string())
        };

        state.blocking_in_progress = true;
        state.block_error = None;
        tasks::block_peer(self.api.clone(), self.tx.clone(), peer_id, reason);
    }

    fn spawn_unblock_peer(&mut self, peer_id: String) {
        tasks::unblock_peer(self.api.clone(), self.tx.clone(), peer_id);
    }

    fn spawn_load_blocklists(&mut self) {
        if self.blocking_state.blocklists_loading {
            return;
        }
        self.blocking_state.blocklists_loading = true;
        self.blocking_state.blocklists_error = None;
        tasks::load_blocklists(self.api.clone(), self.tx.clone());
    }

    fn spawn_subscribe_blocklist(&mut self, state: &mut state::BlockingState) {
        let blocklist_id = state.new_blocklist_id.trim().to_string();
        let maintainer = state.new_blocklist_maintainer.trim().to_string();
        let name = state.new_blocklist_name.trim().to_string();

        if blocklist_id.is_empty() || maintainer.is_empty() || name.is_empty() {
            state.subscribe_error = Some("Blocklist ID, maintainer, and name are required".into());
            return;
        }

        let description = if state.new_blocklist_description.trim().is_empty() {
            None
        } else {
            Some(state.new_blocklist_description.trim().to_string())
        };

        state.subscribing_in_progress = true;
        state.subscribe_error = None;
        tasks::subscribe_blocklist(
            self.api.clone(),
            self.tx.clone(),
            blocklist_id,
            maintainer,
            name,
            description,
            state.new_blocklist_auto_apply,
        );
    }

    fn spawn_unsubscribe_blocklist(&mut self, blocklist_id: String) {
        tasks::unsubscribe_blocklist(self.api.clone(), self.tx.clone(), blocklist_id);
    }

    fn spawn_load_blocklist_entries(&mut self, blocklist_id: String) {
        self.blocking_state.viewing_blocklist_id = Some(blocklist_id.clone());
        self.blocking_state.blocklist_entries_loading = true;
        self.blocking_state.blocklist_entries_error = None;
        tasks::load_blocklist_entries(self.api.clone(), self.tx.clone(), blocklist_id);
    }

    // IP Blocking spawn methods

    fn spawn_load_ip_blocks(&mut self) {
        if self.blocking_state.ip_blocks_loading {
            return;
        }
        self.blocking_state.ip_blocks_loading = true;
        self.blocking_state.ip_blocks_error = None;
        tasks::load_ip_blocks(self.api.clone(), self.tx.clone());
    }

    fn spawn_load_ip_block_stats(&mut self) {
        if self.blocking_state.ip_block_stats_loading {
            return;
        }
        self.blocking_state.ip_block_stats_loading = true;
        tasks::load_ip_block_stats(self.api.clone(), self.tx.clone());
    }

    fn spawn_add_ip_block(&mut self, state: &mut state::BlockingState) {
        let ip_or_range = state.new_ip_block.trim().to_string();
        if ip_or_range.is_empty() {
            state.add_ip_block_error = Some("IP/Range cannot be empty".into());
            return;
        }
        let reason = if state.new_ip_block_reason.trim().is_empty() {
            None
        } else {
            Some(state.new_ip_block_reason.trim().to_string())
        };

        state.adding_ip_block = true;
        state.add_ip_block_error = None;
        tasks::add_ip_block(self.api.clone(), self.tx.clone(), ip_or_range, reason);
    }

    fn spawn_remove_ip_block(&mut self, block_id: i64) {
        tasks::remove_ip_block(self.api.clone(), self.tx.clone(), block_id);
    }

    fn spawn_import_ip_blocks(&mut self, state: &mut state::BlockingState) {
        if state.import_text.trim().is_empty() {
            state.import_error = Some("Import text cannot be empty".into());
            return;
        }

        state.importing_ips = true;
        state.import_error = None;
        tasks::import_ip_blocks(self.api.clone(), self.tx.clone(), state.import_text.clone());
    }

    fn spawn_export_ip_blocks(&mut self) {
        tasks::export_ip_blocks(self.api.clone(), self.tx.clone());
    }

    fn spawn_clear_all_ip_blocks(&mut self) {
        tasks::clear_all_ip_blocks(self.api.clone(), self.tx.clone());
    }

    fn spawn_delete_thread(&mut self, thread_id: String) {
        tasks::delete_thread(self.api.clone(), self.tx.clone(), thread_id);
    }

    fn spawn_ignore_thread(&mut self, thread_id: String) {
        tasks::ignore_thread(self.api.clone(), self.tx.clone(), thread_id, true);
    }

    fn spawn_load_reactions(&mut self, post_id: &str) {
        tasks::load_reactions(self.api.clone(), self.tx.clone(), post_id.to_string());
    }

    fn spawn_add_reaction(&mut self, post_id: &str, emoji: &str) {
        tasks::add_reaction(self.api.clone(), self.tx.clone(), post_id.to_string(), emoji.to_string());
    }

    fn spawn_remove_reaction(&mut self, post_id: &str, emoji: &str) {
        tasks::remove_reaction(self.api.clone(), self.tx.clone(), post_id.to_string(), emoji.to_string());
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

    fn execute_search(&mut self, query: String) {
        // Reuse existing SearchState if present, otherwise create new
        if let ViewState::SearchResults(ref mut state) = self.view {
            state.query = query.clone();
            state.is_loading = true;
            state.error = None;
        } else {
            self.view = ViewState::SearchResults(state::SearchState {
                query: query.clone(),
                results: Vec::new(),
                is_loading: true,
                error: None,
            });
        }
        self.spawn_search(query);
    }

    fn spawn_search(&self, query: String) {
        let tx = self.tx.clone();
        let api = self.api.clone();

        std::thread::spawn(move || {
            let result = api.search(&query, Some(50));
            let _ = tx.send(AppMessage::SearchCompleted { query, result });
        });
    }

    pub fn open_thread_and_scroll_to_post(&mut self, thread_id: String, post_id: String) {
        if let Some(summary) = self.threads.iter().find(|t| t.id == thread_id).cloned() {
            let state = ThreadState {
                summary,
                details: None,
                is_loading: true,
                scroll_to_post: Some(post_id),
                ..Default::default()
            };
            self.view = ViewState::Thread(state);
            self.spawn_load_thread(&thread_id);
        }
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

impl eframe::App for GraphchanApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Store context for video player initialization
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }

        // Handle keyboard input EARLY, before any UI is rendered.
        // This ensures we can consume keys (like Tab) before egui's default widgets see them.
        ui::input::handle_keyboard_input(self, ctx);

        self.process_messages();

        // Request repaint for animation (show for 1 second after refresh starts)
        let show_animation = self.last_refresh_time
            .map(|t| t.elapsed().as_secs_f32() < 1.0)
            .unwrap_or(false);
        if show_animation {
            ctx.request_repaint();
        }

        // Auto-refresh logic - poll every 5 seconds
        if self.auto_refresh_enabled && !self.threads_loading {
            let should_refresh = self.last_refresh_time
                .map(|t| t.elapsed().as_secs() >= 5)
                .unwrap_or(true);

            if should_refresh {
                self.spawn_load_threads();

                // Also refresh current thread if viewing one
                if let ViewState::Thread(state) = &self.view {
                    if !state.is_loading {
                        let thread_id = state.summary.id.clone();
                        self.spawn_refresh_thread(&thread_id);
                    }
                }
            }
        }

        // Recent posts polling
        if !self.recent_posts_loading {
            let should_poll = self.last_recent_posts_refresh
                .map(|t| t.elapsed().as_secs() >= self.recent_posts_poll_interval)
                .unwrap_or(true);

            if should_poll {
                self.spawn_load_recent_posts();
            }
        }

        // Keyboard shortcut: Ctrl+F / Cmd+F for search
        if ctx.input(|i| i.modifiers.command && i.key_pressed(egui::Key::F)) {
            self.search_focused = true;
        }

        egui::TopBottomPanel::top("top_controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Catalog").clicked() {
                    self.view = ViewState::Catalog;
                }
                if ui.button("Messages").clicked() {
                    self.view = ViewState::Messages;
                }
                if ui.button("New Thread").clicked() {
                    self.show_create_thread = true;
                }
                if ui.button("Import").clicked() {
                    self.view = ViewState::Import;
                }
                if ui.button("Following").clicked() {
                    self.view = ViewState::Following;
                }
                if ui.selectable_label(self.show_identity, "Identity").clicked() {
                    self.show_identity = !self.show_identity;
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("⚙ Settings").clicked() {
                        self.view = ViewState::Settings;
                    }

                    ui.add_space(10.0);
                    ui.separator();

                    // Search input
                    if !self.search_query_input.is_empty() && ui.button("✖").clicked() {
                        self.search_query_input.clear();
                    }

                    if !self.search_query_input.is_empty() && ui.button("Search").clicked() {
                        self.execute_search(self.search_query_input.clone());
                    }

                    let search_response = ui.add_sized(
                        egui::vec2(200.0, 20.0),
                        egui::TextEdit::singleline(&mut self.search_query_input)
                            .hint_text("Search posts & files...")
                    );

                    if self.search_focused {
                        search_response.request_focus();
                        self.search_focused = false;
                    }

                    if search_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        if !self.search_query_input.trim().is_empty() {
                            self.execute_search(self.search_query_input.clone());
                        }
                    }

                    ui.label("🔍");

                    ui.add_space(10.0);

                    // Auto-refresh toggle with animated indicator
                    ui.horizontal(|ui| {
                        // Show animation for 1 second after refresh starts
                        let show_animation = self.last_refresh_time
                            .map(|t| t.elapsed().as_secs_f32() < 1.0)
                            .unwrap_or(false);

                        // Animated green dot
                        let (dot_size, dot_color) = if show_animation {
                            // Pulsing animation - faster and more pronounced
                            let elapsed = self.last_refresh_time
                                .map(|t| t.elapsed().as_secs_f32())
                                .unwrap_or(0.0);
                            let pulse = (elapsed * 8.0).sin().abs(); // Faster pulse
                            let size = 5.0 + pulse * 4.0; // More pronounced (5-9px)
                            let brightness = 50 + (pulse * 100.0) as u8; // Pulse brightness too
                            (size, egui::Color32::from_rgb(brightness, 255, brightness))
                        } else if self.auto_refresh_enabled {
                            (6.0, egui::Color32::from_rgb(50, 205, 50)) // Static green
                        } else {
                            (6.0, egui::Color32::GRAY) // Gray when disabled
                        };

                        let (rect, _) = ui.allocate_exact_size(
                            egui::vec2(dot_size, dot_size),
                            egui::Sense::hover()
                        );
                        ui.painter().circle_filled(rect.center(), dot_size / 2.0, dot_color);

                        // Toggle button
                        let button_text = if self.auto_refresh_enabled {
                            "🔄 Auto-Refresh: ON"
                        } else {
                            "🔄 Auto-Refresh: OFF"
                        };

                        if ui.button(button_text).clicked() {
                            self.auto_refresh_enabled = !self.auto_refresh_enabled;
                            if self.auto_refresh_enabled {
                                self.spawn_load_threads();
                            }
                        }
                    });
                });
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
            ViewState::Messages => "messages",
            ViewState::Thread(_) => "thread",
            ViewState::Following => "following",
            ViewState::FollowingCatalog(_) => "following_catalog",
            ViewState::Import => "import",
            ViewState::Settings => "settings",
            ViewState::Conversation(_) => "conversation",
            ViewState::Blocking => "blocking",
            ViewState::SearchResults(_) => "search",
        };

        if view_type == "catalog" {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.render_catalog(ui);
            });
        } else if view_type == "messages" {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui::conversations::render_conversations_list(self, ui, &self.dm_state.conversations.clone());
            });
        } else if view_type == "import" {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui::import::render_import_page(self, ui);
            });
        } else if view_type == "following" {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui::friends::render_friends_page(self, ui);
            });
        } else if view_type == "following_catalog" {
            // Extract peer temporarily
            let peer = if let ViewState::FollowingCatalog(p) = &self.view {
                p.clone()
            } else {
                unreachable!()
            };
            
            egui::CentralPanel::default().show(ctx, |ui| {
                if ui.button("← Back to Following").clicked() {
                    go_back = true;
                }
                ui.add_space(10.0);
                self.render_friend_catalog(ui, &peer);
            });

            if go_back {
                self.view = ViewState::Following;
            }
        } else if view_type == "thread" {
            // Extract thread state temporarily
            let mut temp_state = if let ViewState::Thread(state) = &mut self.view {
                std::mem::replace(
                    state,
                    ThreadState::default(),
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
        } else if view_type == "settings" {
            egui::CentralPanel::default().show(ctx, |ui| {
                self.render_settings(ui);
            });
        } else if view_type == "conversation" {
            // Extract conversation state temporarily
            let mut temp_state = if let ViewState::Conversation(state) = &mut self.view {
                std::mem::replace(
                    state,
                    ConversationState::default(),
                )
            } else {
                unreachable!()
            };

            egui::CentralPanel::default().show(ctx, |ui| {
                ui::conversations::render_conversation(self, ui, &mut temp_state);
            });

            // Restore state
            if let ViewState::Conversation(state) = &mut self.view {
                *state = temp_state;
            }
        } else if view_type == "blocking" {
            let mut temp_state = std::mem::take(&mut self.blocking_state);

            egui::CentralPanel::default().show(ctx, |ui| {
                ui::blocking::render_blocking_page(self, ui, &mut temp_state);
            });

            // Render blocklist entries dialog if viewing one (before restoring state)
            if temp_state.viewing_blocklist_id.is_some() {
                ui::blocking::render_blocklist_entries_dialog(self, ctx, &mut temp_state);
            }

            // Render clear all IP blocks confirmation dialog
            if temp_state.showing_clear_all_confirmation {
                ui::blocking::render_clear_all_ip_blocks_dialog(self, ctx, &mut temp_state);
            }

            self.blocking_state = temp_state;
        } else if view_type == "search" {
            // Extract search state temporarily
            let mut temp_state = if let ViewState::SearchResults(state) = &mut self.view {
                std::mem::replace(
                    state,
                    state::SearchState::default(),
                )
            } else {
                unreachable!()
            };

            egui::CentralPanel::default().show(ctx, |ui| {
                ui::search::render_search_results(self, ui, &mut temp_state);
            });

            // Restore state
            if let ViewState::SearchResults(state) = &mut self.view {
                *state = temp_state;
            }
        }



        self.render_create_thread_dialog(ctx);
        self.render_import_dialog(ctx);
        ui::drawer::render_identity_drawer(self, ctx);
        ui::drawer::render_avatar_cropper(self, ctx);
        self.render_file_viewers(ctx);
        self.render_image_viewers(ctx);
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
