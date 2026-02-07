use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};

use chrono::{DateTime, Utc};
use eframe::egui::{self, Context, TextureHandle};
use egui_commonmark::CommonMarkCache;
use egui_video::AudioDevice;
use log::error;

use crate::api::ApiClient;
// ImageLoader removed; using custom async pipeline
use crate::models::{CreatePostInput, CreateThreadInput, PeerView, ThreadSummary};

mod messages;
mod state;
mod tasks;
mod ui;

// Sub-managers extracted from this file
mod file_viewer;
mod spawners;

// Message handler modules
mod handlers_blocking;
mod handlers_files;
mod handlers_misc;
mod handlers_threads;

use messages::AppMessage;
pub use file_viewer::{FileType, FileDownloadState, FileViewerState, FileViewerContent};
use state::{
    BlockingState, ConversationState, CreateThreadState, DmState, ImporterState, LoadedImage,
    RedditImporterState, ThreadDisplayMode, ThreadState, ViewState,
};

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
    show_global_threads: bool, // Toggle to show/hide global discovery threads
    catalog_topic_filter: Option<String>, // Filter catalog by topic (None = show all)
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
    // Topic management
    subscribed_topics: Vec<String>,
    topics_loading: bool,
    topics_error: Option<String>,
    show_topic_manager: bool,
    new_topic_input: String,
    selected_topics: HashSet<String>, // For create thread dialog
    // Theme customization
    primary_color: egui::Color32,
    theme_dirty: bool, // Flag to reapply theme on next frame
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
            show_global_threads: true, // Default to showing global threads
            catalog_topic_filter: None, // Default to showing all topics
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
            // Topic management
            subscribed_topics: Vec::new(),
            topics_loading: false,
            topics_error: None,
            show_topic_manager: false,
            new_topic_input: String::new(),
            selected_topics: HashSet::new(),
            // Theme customization
            primary_color: egui::Color32::from_rgb(64, 128, 255), // Default blue
            theme_dirty: true, // Apply default theme on first frame
        };
        app.spawn_load_threads();
        app.spawn_load_recent_posts();
        app.spawn_load_peers();
        app.spawn_load_conversations();
        app.spawn_load_blocked_peers();
        app.spawn_load_blocklists();
        app.spawn_load_ip_blocks();
        app.spawn_load_ip_block_stats();
        app.spawn_load_topics();
        app.spawn_load_theme_color();
        app
    }

    fn process_messages(&mut self) {
        messages::process_messages(self);
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
}

impl eframe::App for GraphchanApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
        // Store context for video player initialization
        if self.ctx.is_none() {
            self.ctx = Some(ctx.clone());
        }

        // Apply theme if it changed
        if self.theme_dirty {
            let color_scheme = crate::color_theme::ColorScheme::from_primary(self.primary_color);
            let visuals = color_scheme.to_dark_visuals();
            ctx.set_visuals(visuals);
            self.theme_dirty = false;
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
                if ui.button("Topics").clicked() {
                    self.show_topic_manager = true;
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
        self.render_topic_manager(ctx);
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
