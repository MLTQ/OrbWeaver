use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use chrono::{DateTime, Utc};
use eframe::egui::{self, Align2, Color32, Context, RichText};
use log::error;

use crate::api::ApiClient;
use crate::importer;
use crate::models::{
    CreatePostInput, CreateThreadInput, FileResponse, PostView, ThreadDetails, ThreadSummary,
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
    pending_thread_focus: Option<String>,
}

#[derive(Default)]
struct CreateThreadState {
    title: String,
    body: String,
    submitting: bool,
    error: Option<String>,
}

#[derive(Default)]
struct ImporterState {
    open: bool,
    url: String,
    importing: bool,
    error: Option<String>,
}

enum ViewState {
    Catalog,
    Thread(ThreadState),
}

struct ThreadState {
    summary: ThreadSummary,
    details: Option<ThreadDetails>,
    is_loading: bool,
    error: Option<String>,
    new_post_body: String,
    new_post_error: Option<String>,
    new_post_sending: bool,
    attachments: HashMap<String, Vec<FileResponse>>,
    attachments_loading: HashSet<String>,
    attachments_errors: HashMap<String, String>,
}

enum AppMessage {
    ThreadsLoaded(Result<Vec<ThreadSummary>, anyhow::Error>),
    ThreadLoaded {
        thread_id: String,
        result: Result<ThreadDetails, anyhow::Error>,
    },
    ThreadCreated(Result<ThreadDetails, anyhow::Error>),
    PostCreated {
        thread_id: String,
        result: Result<PostView, anyhow::Error>,
    },
    PostAttachmentsLoaded {
        thread_id: String,
        post_id: String,
        result: Result<Vec<FileResponse>, anyhow::Error>,
    },
    ImportFinished(Result<String, anyhow::Error>),
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
        let client = self.api.clone();
        let tx = self.tx.clone();
        thread::spawn(move || {
            let result = client.list_threads();
            let _ = tx.send(AppMessage::ThreadsLoaded(result));
        });
    }

    fn spawn_load_thread(&mut self, thread_id: &str) {
        let client = self.api.clone();
        let tx = self.tx.clone();
        let id = thread_id.to_string();
        thread::spawn(move || {
            let result = client.get_thread(&id);
            let _ = tx.send(AppMessage::ThreadLoaded {
                thread_id: id,
                result,
            });
        });
    }

    fn spawn_create_thread(&mut self) {
        let title = self.create_thread.title.trim().to_string();
        if title.is_empty() {
            self.create_thread.error = Some("Title cannot be empty".into());
            return;
        }
        let body = self.create_thread.body.trim().to_string();
        let client = self.api.clone();
        let tx = self.tx.clone();
        let mut payload = CreateThreadInput::default();
        payload.title = title;
        if !body.is_empty() {
            payload.body = Some(body);
        }
        self.create_thread.submitting = true;
        self.create_thread.error = None;
        thread::spawn(move || {
            let result = client.create_thread(&payload);
            let _ = tx.send(AppMessage::ThreadCreated(result));
        });
    }

    fn spawn_create_post(&mut self, thread_state: &mut ThreadState) {
        let body = thread_state.new_post_body.trim().to_string();
        if body.is_empty() {
            thread_state.new_post_error = Some("Post body cannot be empty".into());
            return;
        }
        let client = self.api.clone();
        let tx = self.tx.clone();
        let thread_id = thread_state.summary.id.clone();
        let mut payload = CreatePostInput::default();
        payload.thread_id = thread_id.clone();
        payload.body = body;
        thread_state.new_post_sending = true;
        thread_state.new_post_error = None;
        thread::spawn(move || {
            let result = client.create_post(&thread_id, &payload);
            let _ = tx.send(AppMessage::PostCreated { thread_id, result });
        });
    }

    fn spawn_import_fourchan(&mut self) {
        let url = self.importer.url.trim().to_string();
        if url.is_empty() {
            self.importer.error = Some("Paste a full 4chan thread URL".into());
            return;
        }
        let client = self.api.clone();
        let tx = self.tx.clone();
        self.importer.importing = true;
        self.importer.error = None;
        thread::spawn(move || {
            let result = importer::import_fourchan_thread(&client, &url);
            let _ = tx.send(AppMessage::ImportFinished(result));
        });
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

        let client = self.api.clone();
        let tx = self.tx.clone();
        let thread = thread_id.to_string();
        let post = post_id.to_string();
        thread::spawn(move || {
            let result = client.list_post_files(&post);
            let _ = tx.send(AppMessage::PostAttachmentsLoaded {
                thread_id: thread,
                post_id: post,
                result,
            });
        });
    }

    fn process_messages(&mut self) {
        while let Ok(message) = self.rx.try_recv() {
            match message {
                AppMessage::ThreadsLoaded(result) => {
                    self.threads_loading = false;
                    match result {
                        Ok(mut threads) => {
                            threads.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                            self.threads = threads;
                            if let Some(target) = self.pending_thread_focus.clone() {
                                if let Some(summary) =
                                    self.threads.iter().find(|t| t.id == target).cloned()
                                {
                                    self.pending_thread_focus = None;
                                    self.open_thread(summary);
                                }
                            }
                        }
                        Err(err) => {
                            self.threads_error = Some(err.to_string());
                        }
                    }
                }
                AppMessage::ThreadLoaded { thread_id, result } => {
                    let mut post_ids = Vec::new();
                    if let ViewState::Thread(state) = &mut self.view {
                        if state.summary.id == thread_id {
                            state.is_loading = false;
                            match result {
                                Ok(details) => {
                                    post_ids = details.posts.iter().map(|p| p.id.clone()).collect();
                                    state.summary = details.thread.clone();
                                    state.details = Some(details);
                                    state.error = None;
                                    state.attachments.clear();
                                    state.attachments_errors.clear();
                                    state.attachments_loading.clear();
                                }
                                Err(err) => {
                                    state.error = Some(err.to_string());
                                }
                            }
                        }
                    }
                    for post_id in post_ids {
                        self.spawn_load_attachments_for_post(&thread_id, &post_id);
                    }
                }
                AppMessage::ThreadCreated(result) => {
                    self.create_thread.submitting = false;
                    match result {
                        Ok(details) => {
                            self.create_thread = CreateThreadState::default();
                            self.show_create_thread = false;
                            self.info_banner = Some("Thread created".into());
                            let summary = details.thread.clone();
                            let mut state = ThreadState {
                                summary: summary.clone(),
                                details: Some(details.clone()),
                                is_loading: false,
                                error: None,
                                new_post_body: String::new(),
                                new_post_error: None,
                                new_post_sending: false,
                                attachments: HashMap::new(),
                                attachments_loading: HashSet::new(),
                                attachments_errors: HashMap::new(),
                            };
                            for post in &details.posts {
                                state
                                    .attachments
                                    .entry(post.id.clone())
                                    .or_insert_with(Vec::new);
                            }
                            self.view = ViewState::Thread(state);
                            let post_ids: Vec<String> =
                                details.posts.iter().map(|p| p.id.clone()).collect();
                            for post_id in post_ids {
                                self.spawn_load_attachments_for_post(&summary.id, &post_id);
                            }
                            self.spawn_load_threads();
                        }
                        Err(err) => {
                            self.create_thread.error = Some(err.to_string());
                        }
                    }
                }
                AppMessage::PostCreated { thread_id, result } => {
                    if let ViewState::Thread(state) = &mut self.view {
                        if state.summary.id == thread_id {
                            state.new_post_sending = false;
                            match result {
                                Ok(post) => {
                                    if let Some(details) = &mut state.details {
                                        details.posts.push(post.clone());
                                    }
                                    state
                                        .attachments
                                        .entry(post.id.clone())
                                        .or_insert_with(Vec::new);
                                    state.new_post_body.clear();
                                    state.new_post_error = None;
                                    self.info_banner = Some("Post published".into());
                                }
                                Err(err) => {
                                    state.new_post_error = Some(err.to_string());
                                }
                            }
                        }
                    }
                }
                AppMessage::PostAttachmentsLoaded {
                    thread_id,
                    post_id,
                    result,
                } => {
                    if let ViewState::Thread(state) = &mut self.view {
                        if state.summary.id == thread_id {
                            state.attachments_loading.remove(&post_id);
                            match result {
                                Ok(files) => {
                                    state.attachments.insert(post_id.clone(), files);
                                    state.attachments_errors.remove(&post_id);
                                }
                                Err(err) => {
                                    state
                                        .attachments_errors
                                        .insert(post_id.clone(), err.to_string());
                                }
                            }
                        }
                    }
                }
                AppMessage::ImportFinished(result) => {
                    self.importer.importing = false;
                    match result {
                        Ok(thread_id) => {
                            self.importer.open = false;
                            self.importer.url.clear();
                            self.importer.error = None;
                            self.info_banner = Some("Imported thread from 4chan".into());
                            self.pending_thread_focus = Some(thread_id.clone());
                            self.spawn_load_threads();
                        }
                        Err(err) => {
                            self.importer.error = Some(err.to_string());
                        }
                    }
                }
            }
        }
    }

    fn render_catalog(&mut self, ui: &mut egui::Ui) {
        if self.threads_loading && self.threads.is_empty() {
            ui.add(egui::Spinner::new());
        }
        if let Some(err) = &self.threads_error {
            ui.colored_label(Color32::LIGHT_RED, err);
            if ui.button("Retry").clicked() {
                self.spawn_load_threads();
            }
            ui.separator();
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            if self.threads.is_empty() && !self.threads_loading {
                ui.label("No threads yet. Create one to get started.");
            }
            for thread in &self.threads {
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .margin(egui::vec2(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let title = if thread.title.is_empty() {
                                "(untitled thread)"
                            } else {
                                &thread.title
                            };
                            if ui.button(RichText::new(title).strong()).clicked() {
                                self.open_thread(thread.clone());
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(format_timestamp(&thread.created_at));
                                },
                            );
                        });
                        if let Some(peer) = &thread.creator_peer_id {
                            ui.label(format!("Created by {peer}"));
                        }
                    });
            }
        });
    }

    fn open_thread(&mut self, summary: ThreadSummary) {
        let thread_id = summary.id.clone();
        self.view = ViewState::Thread(ThreadState {
            summary,
            details: None,
            is_loading: true,
            error: None,
            new_post_body: String::new(),
            new_post_error: None,
            new_post_sending: false,
            attachments: HashMap::new(),
            attachments_loading: HashSet::new(),
            attachments_errors: HashMap::new(),
        });
        self.spawn_load_thread(&thread_id);
    }

    fn render_thread_view(&mut self, ui: &mut egui::Ui, state: &mut ThreadState) -> bool {
        let mut go_back = false;
        ui.horizontal(|ui| {
            if ui.button("← Back to catalog").clicked() {
                go_back = true;
            }
            ui.separator();
            ui.label(RichText::new(&state.summary.title).heading());
        });

        if state.is_loading {
            ui.add(egui::Spinner::new());
            return go_back;
        }

        if let Some(err) = &state.error {
            ui.colored_label(Color32::LIGHT_RED, err);
            if ui.button("Retry").clicked() {
                state.is_loading = true;
                state.error = None;
                self.spawn_load_thread(&state.summary.id);
            }
            return go_back;
        }

        if let Some(details) = &state.details {
            egui::ScrollArea::vertical()
                .id_source("thread-posts")
                .show(ui, |ui| {
                    for post in &details.posts {
                        egui::Frame::group(ui.style())
                            .fill(ui.visuals().extreme_bg_color)
                            .margin(egui::vec2(12.0, 8.0))
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(&post.id).monospace());
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            ui.label(format_timestamp(&post.created_at));
                                        },
                                    );
                                });
                                if let Some(author) = &post.author_peer_id {
                                    ui.label(format!("Author: {author}"));
                                }
                                ui.separator();
                                ui.label(&post.body);
                                if !post.parent_post_ids.is_empty() {
                                    ui.add_space(4.0);
                                    ui.label(format!(
                                        "Replies to: {}",
                                        post.parent_post_ids.join(", ")
                                    ));
                                }
                                ui.add_space(6.0);
                                self.render_attachments(ui, state, post);
                            });
                    }
                });
        }

        ui.separator();
        ui.collapsing("Reply", |ui| {
            if let Some(err) = &state.new_post_error {
                ui.colored_label(Color32::LIGHT_RED, err);
            }
            ui.add(
                egui::TextEdit::multiline(&mut state.new_post_body)
                    .desired_rows(4)
                    .hint_text("Write a reply..."),
            );
            ui.horizontal(|ui| {
                if state.new_post_sending {
                    ui.add(egui::Spinner::new());
                } else if ui.button("Post").clicked() {
                    self.spawn_create_post(state);
                }
                if ui.button("Cancel").clicked() {
                    state.new_post_body.clear();
                    state.new_post_error = None;
                }
            });
        });

        go_back
    }

    fn render_attachments(&mut self, ui: &mut egui::Ui, state: &mut ThreadState, post: &PostView) {
        if !state.attachments.contains_key(&post.id)
            && !state.attachments_loading.contains(&post.id)
            && !state.attachments_errors.contains_key(&post.id)
        {
            self.spawn_load_attachments_for_post(&state.summary.id, &post.id);
        }

        if let Some(err) = state.attachments_errors.get(&post.id) {
            ui.colored_label(Color32::LIGHT_RED, err);
            if ui.small_button("Retry attachments").clicked() {
                state.attachments_errors.remove(&post.id);
                self.spawn_load_attachments_for_post(&state.summary.id, &post.id);
            }
            return;
        }

        if state.attachments_loading.contains(&post.id) {
            ui.horizontal(|ui| {
                ui.add(egui::Spinner::new());
                ui.label("Loading attachments…");
            });
            return;
        }

        if let Some(files) = state.attachments.get(&post.id) {
            if files.is_empty() {
                return;
            }
            ui.label(RichText::new("Attachments").strong());
            for file in files {
                let name = file.original_name.as_deref().unwrap_or("attachment");
                let label = if file.present {
                    name.to_string()
                } else {
                    format!("{name} (remote)")
                };
                let url = self.api.download_url(&file.id);
                ui.hyperlink_to(label, url);
            }
        }
    }

    fn render_create_thread_dialog(&mut self, ctx: &Context) {
        if !self.show_create_thread {
            return;
        }
        let mut open = self.show_create_thread;
        egui::Window::new("Create Thread")
            .open(&mut open)
            .default_width(400.0)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                if let Some(err) = &self.create_thread.error {
                    ui.colored_label(Color32::LIGHT_RED, err);
                }
                ui.label("Title");
                ui.text_edit_singleline(&mut self.create_thread.title);
                ui.add_space(6.0);
                ui.label("Body (optional)");
                ui.add(
                    egui::TextEdit::multiline(&mut self.create_thread.body)
                        .desired_rows(6)
                        .hint_text("Write the opening post..."),
                );
                ui.add_space(12.0);
                ui.horizontal(|ui| {
                    if self.create_thread.submitting {
                        ui.add(egui::Spinner::new());
                    } else if ui.button("Create").clicked() {
                        self.spawn_create_thread();
                    }
                    if ui.button("Cancel").clicked() {
                        self.show_create_thread = false;
                        self.create_thread = CreateThreadState::default();
                    }
                });
            });
        self.show_create_thread = open;
    }

    fn render_import_dialog(&mut self, ctx: &Context) {
        if !self.importer.open {
            return;
        }
        let mut open = self.importer.open;
        egui::Window::new("Import from 4chan")
            .open(&mut open)
            .default_width(420.0)
            .anchor(Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.label("Paste a thread URL (e.g. https://boards.4chan.org/g/thread/123456789)");
                ui.text_edit_singleline(&mut self.importer.url);
                if let Some(err) = &self.importer.error {
                    ui.colored_label(Color32::LIGHT_RED, err);
                }
                ui.add_space(8.0);
                ui.horizontal(|ui| {
                    if self.importer.importing {
                        ui.add(egui::Spinner::new());
                    } else if ui.button("Import").clicked() {
                        self.spawn_import_fourchan();
                    }
                    if ui.button("Close").clicked() {
                        open = false;
                    }
                });
            });
        self.importer.open = open;
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

        egui::CentralPanel::default().show(ctx, |ui| match &mut self.view {
            ViewState::Catalog => self.render_catalog(ui),
            ViewState::Thread(state) => {
                let go_back = self.render_thread_view(ui, state);
                if go_back {
                    self.view = ViewState::Catalog;
                }
            }
        });

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
