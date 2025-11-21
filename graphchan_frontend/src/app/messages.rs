use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::models::{FileResponse, PostView, ThreadDetails, ThreadSummary};

use super::state::{
    CreateThreadState, GraphNode, LoadedImage, ThreadDisplayMode, ThreadState, ViewState,
};
use super::ui::graph::build_initial_graph;
use super::GraphchanApp;

pub enum AppMessage {
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
    ImageLoaded {
        file_id: String,
        result: Result<LoadedImage, String>,
    },
}

pub(super) fn process_messages(app: &mut GraphchanApp) {
    while let Ok(message) = app.rx.try_recv() {
        match message {
            AppMessage::ThreadsLoaded(result) => {
                app.threads_loading = false;
                match result {
                    Ok(mut threads) => {
                        threads.sort_by(|a, b| b.created_at.cmp(&a.created_at));
                        app.threads = threads;
                        if let Some(target) = app.pending_thread_focus.clone() {
                            if let Some(summary) =
                                app.threads.iter().find(|t| t.id == target).cloned()
                            {
                                app.pending_thread_focus = None;
                                app.open_thread(summary);
                            }
                        }
                    }
                    Err(err) => {
                        app.threads_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::ThreadLoaded { thread_id, result } => {
                let mut post_ids = Vec::new();
                if let ViewState::Thread(state) = &mut app.view {
                    if state.summary.id == thread_id {
                        state.is_loading = false;
                        match result {
                            Ok(details) => {
                                post_ids = details.posts.iter().map(|p| p.id.clone()).collect();
                                state.summary = details.thread.clone();
                                state.graph_nodes = build_initial_graph(&details.posts);
                                state.chronological_nodes = HashMap::new();
                                state.sim_start_time = None;
                                state.details = Some(details);
                                state.graph_zoom = 1.0;
                                state.graph_offset = egui::vec2(0.0, 0.0);
                                state.graph_dragging = false;
                                state.error = None;
                                state.attachments.clear();
                                state.attachments_errors.clear();
                                state.attachments_loading.clear();
                                state.reply_to.clear();
                                state.repulsion_force = 500.0;
                                state.sim_paused = false;
                            }
                            Err(err) => {
                                state.error = Some(err.to_string());
                            }
                        }
                    }
                }
                for post_id in post_ids {
                    app.spawn_load_attachments_for_post(&thread_id, &post_id);
                }
            }
            AppMessage::ThreadCreated(result) => {
                app.create_thread.submitting = false;
                match result {
                    Ok(details) => {
                        app.create_thread = CreateThreadState::default();
                        app.show_create_thread = false;
                        app.info_banner = Some("Thread created".into());
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
                            display_mode: ThreadDisplayMode::List,
                            last_layout_mode: None,
                            graph_nodes: build_initial_graph(&details.posts),
                            chronological_nodes: HashMap::new(),
                            sim_start_time: None,
                            selected_post: None,
                            graph_zoom: 1.0,
                            graph_offset: egui::vec2(0.0, 0.0),
                            graph_dragging: false,
                            reply_to: Vec::new(),
                            time_bin_seconds: 60, // Default to 1 minute bins
                            locked_hover_post: None,
                            repulsion_force: 500.0,
                            sim_paused: false,
                        };
                        for post in &details.posts {
                            state
                                .attachments
                                .entry(post.id.clone())
                                .or_insert_with(Vec::new);
                        }
                        app.view = ViewState::Thread(state);
                        let post_ids: Vec<String> =
                            details.posts.iter().map(|p| p.id.clone()).collect();
                        for post_id in post_ids {
                            app.spawn_load_attachments_for_post(&summary.id, &post_id);
                        }
                        app.spawn_load_threads();
                    }
                    Err(err) => {
                        app.create_thread.error = Some(err.to_string());
                    }
                }
            }
            AppMessage::PostCreated { thread_id, result } => {
                let mut attachment_target: Option<String> = None;
                if let ViewState::Thread(state) = &mut app.view {
                    if state.summary.id == thread_id {
                        state.new_post_sending = false;
                        match result {
                            Ok(post) => {
                                state.new_post_body.clear();
                                state.new_post_error = None;
                                state.details.as_mut().map(|d| d.posts.push(post.clone()));
                                state
                                    .attachments
                                    .entry(post.id.clone())
                                    .or_insert_with(Vec::new);
                                state.graph_nodes.insert(
                                    post.id.clone(),
                                    GraphNode {
                                        pos: egui::pos2(0.5, 0.1),
                                        vel: egui::vec2(0.0, 0.0),
                                        size: egui::vec2(220.0, 140.0),
                                        dragging: false,
                                    },
                                );
                                attachment_target = Some(post.id.clone());
                                app.info_banner = Some("Post published".into());
                                state.reply_to.clear();
                            }
                            Err(err) => {
                                state.new_post_error = Some(err.to_string());
                            }
                        }
                    }
                }
                if let Some(post_id) = attachment_target {
                    app.spawn_load_attachments_for_post(&thread_id, &post_id);
                }
            }
            AppMessage::PostAttachmentsLoaded {
                thread_id,
                post_id,
                result,
            } => {
                let mut downloads: Vec<(String, String)> = Vec::new();
                let mut base_url: Option<String> = None;

                if let ViewState::Thread(state) = &mut app.view {
                    if state.summary.id != thread_id {
                        continue;
                    }
                    state.attachments_loading.remove(&post_id);
                    match result {
                        Ok(files) => {
                            for file in &files {
                                if file.present {
                                    if let Some(mime) = &file.mime {
                                        let already_have = app.image_loading.contains(&file.id)
                                            || app.image_textures.contains_key(&file.id)
                                            || app.image_errors.contains_key(&file.id);
                                        if mime.starts_with("image/") && !already_have {
                                            let base_ref = base_url.get_or_insert_with(|| {
                                                app.api.base_url().to_string()
                                            });
                                            let url = super::resolve_download_url(
                                                base_ref,
                                                file.download_url.as_deref(),
                                                &file.id,
                                            );
                                            downloads.push((file.id.clone(), url));
                                        }
                                    }
                                }
                            }
                            state.attachments.insert(post_id.clone(), files);
                            state.attachments_errors.remove(&post_id);
                            base_url.get_or_insert_with(|| app.api.base_url().to_string());
                        }
                        Err(err) => {
                            state
                                .attachments_errors
                                .insert(post_id.clone(), err.to_string());
                        }
                    }
                }

                if base_url.is_some() {
                    for (file_id, url) in downloads {
                        app.spawn_download_image(&file_id, &url);
                    }
                }
            }
            AppMessage::ImportFinished(result) => {
                app.importer.importing = false;
                match result {
                    Ok(thread_id) => {
                        app.importer.open = false;
                        app.importer.url.clear();
                        app.importer.error = None;
                        app.info_banner = Some("Imported thread from 4chan".into());
                        app.pending_thread_focus = Some(thread_id.clone());
                        app.spawn_load_threads();
                    }
                    Err(err) => {
                        app.importer.error = Some(format!("{err:#}"));
                    }
                }
            }
            AppMessage::ImageLoaded { file_id, result } => {
                app.image_loading.remove(&file_id);
                match result {
                    Ok(img) => {
                        app.image_pending.insert(file_id, img);
                    }
                    Err(e) => {
                        app.image_errors.insert(file_id, e);
                    }
                }
                // Download completed, process next item in queue
                app.on_download_complete();
            }
        }
    }
}
