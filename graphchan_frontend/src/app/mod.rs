use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};

use chrono::{DateTime, Utc};
use eframe::egui::{self, Context, TextureHandle};
use log::error;

use crate::api::ApiClient;
// ImageLoader removed; using custom async pipeline
use crate::models::{CreatePostInput, CreateThreadInput, ThreadSummary};

mod messages;
mod state;
mod tasks;
mod ui;

use messages::AppMessage;
use state::{
    CreateThreadState, ImporterState, LoadedImage, ThreadDisplayMode, ThreadState, ViewState,
};

// Maximum number of concurrent image downloads to prevent overwhelming the backend
const MAX_CONCURRENT_DOWNLOADS: usize = 20;

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

impl GraphchanApp {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let default_url = std::env::var("GRAPHCHAN_API_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
        let api = ApiClient::new(default_url.clone()).unwrap_or_else(|err| {
            error!("failed to initialise API client: {err}");
            ApiClient::new("http://127.0.0.1:8080").expect("fallback API client")
        });
        let (tx, rx) = mpsc::channel();

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
        };
        app.spawn_load_threads();
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
        tasks::create_thread(self.api.clone(), self.tx.clone(), payload);
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

    fn spawn_load_attachments_for_post(&mut self, thread_id: &str, post_id: &str) {
        let should_spawn = if let ViewState::Thread(state) = &mut self.view {
            if state.summary.id != thread_id {
                return;
            }
            if state.attachments.contains_key(post_id)
                || state.attachments_loading.contains(post_id)
            {
                return;
            }
            state.attachments_loading.insert(post_id.to_string());
            true
        } else {
            false
        };

        if !should_spawn {
            return;
        }

        tasks::load_attachments(
            self.api.clone(),
            self.tx.clone(),
            thread_id.to_string(),
            post_id.to_string(),
        );
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
}

impl eframe::App for GraphchanApp {
    fn update(&mut self, ctx: &Context, _frame: &mut eframe::Frame) {
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

            egui::CentralPanel::default().show(ctx, |ui| {
                go_back = self.render_thread_view(ui, &mut temp_state);
            });

            // Put it back
            if let ViewState::Thread(state) = &mut self.view {
                *state = temp_state;
            }
        }

        if go_back {
            self.view = ViewState::Catalog;
        }

        self.render_image_viewers(ctx);

        self.render_create_thread_dialog(ctx);
        self.render_import_dialog(ctx);
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
