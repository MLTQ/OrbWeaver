use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;

use chrono::{DateTime, Utc};
use eframe::egui::{self, Align2, Color32, Context, RichText, TextureHandle};
use log::error;

use crate::api::ApiClient;
// ImageLoader removed; using custom async pipeline
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
    image_textures: HashMap<String, TextureHandle>,
    image_loading: HashSet<String>,
    image_pending: HashMap<String, LoadedImage>,
    image_errors: HashMap<String, String>,
    image_viewers: HashMap<String, bool>,
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
    ImageLoaded { file_id: String, result: Result<LoadedImage, String> },
}

#[derive(Clone)]
struct LoadedImage { pub size: [usize;2], pub pixels: Vec<u8> }

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
                                    // Collect downloads outside borrow scope
                                    let mut to_download = Vec::new();
                                    for f in &files {
                                        if let Some(mime) = &f.mime {
                                            if mime.starts_with("image/") && f.present {
                                                if !self.image_loading.contains(&f.id) && !self.image_textures.contains_key(&f.id) && !self.image_errors.contains_key(&f.id) {
                                                    to_download.push(f.id.clone());
                                                }
                                            }
                                        }
                                    }
                                    state.attachments.insert(post_id.clone(), files);
                                    state.attachments_errors.remove(&post_id);
                                    drop(state);
                                    for id in to_download { let url = format!("{}/files/{}", self.api.base_url(), id); self.spawn_download_image(&id, &url); }
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
                AppMessage::ImageLoaded { file_id, result } => {
                    self.image_loading.remove(&file_id);
                    match result {
                        Ok(img) => { self.image_pending.insert(file_id, img); }
                        Err(e) => { self.image_errors.insert(file_id, e); }
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
            
            let mut thread_to_open: Option<ThreadSummary> = None;
            
            for thread in &self.threads {
                egui::Frame::group(ui.style())
                    .fill(ui.visuals().extreme_bg_color)
                    .inner_margin(egui::vec2(12.0, 8.0))
                    .show(ui, |ui| {
                        ui.horizontal(|ui| {
                            let title = if thread.title.is_empty() {
                                "(untitled thread)"
                            } else {
                                &thread.title
                            };
                            if ui.button(RichText::new(title).strong()).clicked() {
                                thread_to_open = Some(thread.clone());
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
            
            if let Some(thread) = thread_to_open {
                self.open_thread(thread);
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
        let mut should_retry = false;
        let mut should_create_post = false;
        
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
                should_retry = true;
            }
            if should_retry {
                state.is_loading = true;
                state.error = None;
                let thread_id = state.summary.id.clone();
                self.spawn_load_thread(&thread_id);
            }
            return go_back;
        }

        if let Some(details) = &state.details {
            let posts_clone = details.posts.clone();
            let thread_id = state.summary.id.clone();
            let attachments = &state.attachments;
            let attachments_loading = &state.attachments_loading;
            let attachments_errors = &state.attachments_errors;
            let api_base = self.api.base_url().to_string();
            
            egui::ScrollArea::vertical()
                .id_source("thread-posts")
                .show(ui, |ui| {
                    for post in &posts_clone {
                        egui::Frame::group(ui.style())
                            .fill(ui.visuals().extreme_bg_color)
                            .inner_margin(egui::vec2(12.0, 8.0))
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
                                
                                // Render attachments inline
                                if let Some(err) = attachments_errors.get(&post.id) {
                                    ui.colored_label(Color32::LIGHT_RED, err);
                                } else if attachments_loading.contains(&post.id) {
                                    ui.horizontal(|ui| {
                                        ui.add(egui::Spinner::new());
                                        ui.label("Loading attachments…");
                                    });
                                } else if let Some(files) = attachments.get(&post.id) {
                                    if !files.is_empty() {
                                        ui.label(RichText::new("Attachments").strong());
                                        for file in files {
                                            self.render_file_attachment(ui, file, &api_base, &post.id);
                                        }
                                    }
                                }
                            });
                            
                        // Queue attachment loading if needed
                        if !state.attachments.contains_key(&post.id)
                            && !state.attachments_loading.contains(&post.id)
                            && !state.attachments_errors.contains_key(&post.id)
                        {
                            self.spawn_load_attachments_for_post(&thread_id, &post.id);
                        }
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
                    should_create_post = true;
                }
                if ui.button("Cancel").clicked() {
                    state.new_post_body.clear();
                    state.new_post_error = None;
                }
            });
        });
        
        if should_create_post {
            self.spawn_create_post(state);
        }

        go_back
    }

    fn spawn_download_image(&mut self, file_id: &str, url: &str) {
        self.image_loading.insert(file_id.to_string());
        let tx = self.tx.clone();
        let id = file_id.to_string();
        let url_owned = url.to_string();
        thread::spawn(move || {
            let result = (|| {
                let resp = reqwest::blocking::get(&url_owned).map_err(|e| e.to_string())?;
                let bytes = resp.bytes().map_err(|e| e.to_string())?;
                let dyn_img = image::load_from_memory(&bytes).map_err(|e| e.to_string())?;
                let rgba = dyn_img.to_rgba8();
                let size = [dyn_img.width() as usize, dyn_img.height() as usize];
                Ok(LoadedImage { size, pixels: rgba.as_flat_samples().as_slice().to_vec() })
            })();
            let _ = tx.send(AppMessage::ImageLoaded { file_id: id, result });
        });
    }

    fn render_file_attachment(&mut self, ui: &mut egui::Ui, file: &FileResponse, api_base: &str, _post_id: &str) {
        let download_url = format!("{}/files/{}", api_base, file.id);
        let name = file.original_name.as_deref().unwrap_or("attachment");
        let mime = file.mime.as_deref().unwrap_or("");
        
        // Check if it's an image
        let is_image = mime.starts_with("image/");
        
        if is_image && file.present {
            if let Some(tex) = self.image_textures.get(&file.id) { // display cached texture
                let response = ui.add(egui::Image::from_texture(tex).max_width(400.0).sense(egui::Sense::click()));
                let hovered = response.hovered();
                let clicked = response.clicked();
                if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); response.clone().on_hover_text(name); }
                if clicked { self.image_viewers.insert(file.id.clone(), true); }
            } else if let Some(pending) = self.image_pending.remove(&file.id) {
                let color = egui::ColorImage::from_rgba_unmultiplied(pending.size, &pending.pixels);
                let tex = ui.ctx().load_texture(&file.id, color, egui::TextureOptions::default());
                self.image_textures.insert(file.id.clone(), tex.clone());
                let response = ui.add(egui::Image::from_texture(&tex).max_width(400.0).sense(egui::Sense::click()));
                let hovered = response.hovered();
                let clicked = response.clicked();
                if hovered { ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand); response.clone().on_hover_text(name); }
                if clicked { self.image_viewers.insert(file.id.clone(), true); }
            } else if self.image_errors.get(&file.id).is_some() {
                ui.colored_label(Color32::LIGHT_RED, format!("Image failed: {}", self.image_errors[&file.id]));
                ui.hyperlink_to(name, download_url);
            } else {
                if !self.image_loading.contains(&file.id) {
                    self.spawn_download_image(&file.id, &download_url);
                }
                ui.horizontal(|ui| { ui.add(egui::Spinner::new()); ui.label(format!("Loading {}", name)); });
            }
        } else {
            let label = if file.present { name.to_string() } else { format!("{} (remote)", name) };
            ui.hyperlink_to(label, download_url);
        }
    }

    fn render_create_thread_dialog(&mut self, ctx: &Context) {
        if !self.show_create_thread {
            return;
        }
        
        let mut should_close = false;
        let mut should_submit = false;
        
        egui::Window::new("Create Thread")
            .open(&mut self.show_create_thread)
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
                        should_submit = true;
                    }
                    if ui.button("Cancel").clicked() {
                        should_close = true;
                    }
                });
            });
            
        if should_submit {
            self.spawn_create_thread();
        }
        if should_close {
            self.show_create_thread = false;
            self.create_thread = CreateThreadState::default();
        }
    }

    fn render_import_dialog(&mut self, ctx: &Context) {
        if !self.importer.open {
            return;
        }
        
        let mut should_close = false;
        let mut should_import = false;
        
        egui::Window::new("Import from 4chan")
            .open(&mut self.importer.open)
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
                        should_import = true;
                    }
                    if ui.button("Close").clicked() {
                        should_close = true;
                    }
                });
            });
            
        if should_import {
            self.spawn_import_fourchan();
        }
        if should_close {
            self.importer.open = false;
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
                std::mem::replace(state, ThreadState {
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
                })
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

        // Render image viewer windows
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
                        if size.x > max_w { let scale = max_w / size.x; size.x *= scale; size.y *= scale; }
                        ui.image(tex);
                    } else {
                        ui.label("Loading image...");
                    }
                });
            if !*open { closed.push(id.clone()); }
        }
        for id in closed { self.image_viewers.remove(&id); }

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

fn load_image_sync(url: &str) -> Result<egui::ColorImage, String> {
    // removed unused: use image::ImageFormat;
    use reqwest::blocking::Client;
    
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| format!("Failed to create HTTP client: {}", e))?;
    
    let response = client
        .get(url)
        .send()
        .map_err(|e| format!("Failed to fetch: {}", e))?;
    
    let bytes = response
        .bytes()
        .map_err(|e| format!("Failed to read bytes: {}", e))?;
    
    let image = image::load_from_memory(&bytes)
        .map_err(|e| format!("Failed to decode image: {}", e))?;
    
    let size = [image.width() as _, image.height() as _];
    let image_buffer = image.to_rgba8();
    let pixels = image_buffer.as_flat_samples();
    
    Ok(egui::ColorImage::from_rgba_unmultiplied(
        size,
        pixels.as_slice(),
    ))
}
