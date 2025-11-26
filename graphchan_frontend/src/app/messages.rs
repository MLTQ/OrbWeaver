use std::collections::{HashMap, HashSet};

use eframe::egui;
use log::error;

use crate::models::{FileResponse, PeerView, PostView, ThreadDetails, ThreadSummary};

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
    IdentityLoaded(Result<PeerView, anyhow::Error>),
    PeersLoaded(Result<Vec<PeerView>, anyhow::Error>),
    PeerAdded(Result<PeerView, anyhow::Error>),
    AvatarUploaded(Result<(), anyhow::Error>),
    ProfileUpdated(Result<(), anyhow::Error>),
    ThreadFilesSelected(Vec<std::path::PathBuf>),
    TextFileLoaded {
        file_id: String,
        result: Result<String, String>,
    },
    MediaFileLoaded {
        file_id: String,
        result: Result<Vec<u8>, String>,
    },
    PdfFileLoaded {
        file_id: String,
        result: Result<Vec<u8>, String>,
    },
    FileSaved {
        file_id: String,
        result: Result<(), String>,
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
                                
                                // Update global peer cache
                                for peer in &details.peers {
                                    app.peers.insert(peer.id.clone(), peer.clone());
                                }
                                
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
                                state.draft_attachments.clear();
                                
                                // Populate attachments from posts
                                // Populate attachments from posts
                                let mut downloads = Vec::new();
                                if let Some(details) = &state.details {
                                    for post in &details.posts {
                                        if !post.files.is_empty() {
                                            state.attachments.insert(post.id.clone(), post.files.clone());
                                            
                                            // Trigger image downloads
                                            for file in &post.files {
                                                if let Some(mime) = &file.mime {
                                                    if mime.starts_with("image/") {
                                                        let url = if let Some(blob_id) = &file.blob_id {
                                                            crate::app::resolve_blob_url(&app.base_url_input, blob_id)
                                                        } else {
                                                            crate::app::resolve_file_url(&app.base_url_input, &file.id)
                                                        };
                                                        downloads.push((file.id.clone(), url));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                                
                                // Spawn downloads after releasing borrow
                                for (file_id, url) in downloads {
                                    app.spawn_load_image(&file_id, &url);
                                }
                            }
                            Err(err) => {
                                state.error = Some(err.to_string());
                            }
                        }
                    }
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
                            draft_attachments: Vec::new(),
                        };
                        for post in &details.posts {
                            state
                                .attachments
                                .entry(post.id.clone())
                                .or_insert_with(Vec::new);
                        }
                        app.view = ViewState::Thread(state);
                        // Attachments are already populated in ThreadDetails
                        app.spawn_load_threads();
                        app.spawn_load_threads();
                    }
                    Err(err) => {
                        app.create_thread.error = Some(err.to_string());
                    }
                }
            }
            AppMessage::PostCreated { thread_id, result } => {
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
                                        pinned: false,
                                    },
                                );
                                app.info_banner = Some("Post published".into());
                                let mut downloads = Vec::new();
                                if !post.files.is_empty() {
                                    state.attachments.insert(post.id.clone(), post.files.clone());
                                    // Trigger image downloads
                                    for file in &post.files {
                                        if let Some(mime) = &file.mime {
                                            if mime.starts_with("image/") {
                                                let url = if let Some(blob_id) = &file.blob_id {
                                                    crate::app::resolve_blob_url(&app.base_url_input, blob_id)
                                                } else {
                                                    crate::app::resolve_file_url(&app.base_url_input, &file.id)
                                                };
                                                downloads.push((file.id.clone(), url));
                                            }
                                        }
                                    }
                                }
                                
                                for (file_id, url) in downloads {
                                    app.spawn_load_image(&file_id, &url);
                                }
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
                        error!("Failed to load image {}: {}", file_id, e);
                        app.image_errors.insert(file_id, e);
                    }
                }
                // Download completed, process next item in queue
                app.on_download_complete();
            }
            AppMessage::IdentityLoaded(result) => {
                let state = &mut app.identity_state;
                match result {
                    Ok(peer) => {
                        state.local_peer = Some(peer);
                        state.error = None;
                    }
                    Err(err) => {
                        state.error = Some(format!("Failed to load identity: {err}"));
                    }
                }
            }
            AppMessage::PeersLoaded(result) => {
                match result {
                    Ok(peers) => {
                        for peer in peers {
                            app.peers.insert(peer.id.clone(), peer);
                        }
                    }
                    Err(err) => {
                        error!("Failed to load peers: {err}");
                    }
                }
            }
            AppMessage::AvatarUploaded(result) => {
                let state = &mut app.identity_state;
                state.uploading = false;
                match result {
                    Ok(()) => {
                        state.avatar_path = None;
                        // Reload identity to get new avatar ID
                        app.spawn_load_identity();
                    }
                    Err(err) => {
                        state.error = Some(format!("Failed to upload avatar: {err}"));
                    }
                }
            }
            AppMessage::ProfileUpdated(result) => {
                let state = &mut app.identity_state;
                state.uploading = false;
                match result {
                    Ok(()) => {
                        app.spawn_load_identity();
                    }
                    Err(err) => {
                        state.error = Some(format!("Failed to update profile: {err}"));
                    }
                }
            }
            AppMessage::PeerAdded(result) => {
                let state = &mut app.identity_state;
                state.adding_peer = false;
                match result {
                    Ok(peer) => {
                        app.peers.insert(peer.id.clone(), peer.clone());
                        app.info_banner = Some(format!("Added peer: {}", peer.username.as_deref().unwrap_or("Unknown")));
                        state.friendcode_input.clear();
                        state.error = None;
                    }
                    Err(err) => {
                        state.error = Some(format!("Failed to add peer: {err}"));
                    }
                }
            }
            AppMessage::ThreadFilesSelected(files) => {
                app.create_thread.files.extend(files);
            }
            AppMessage::TextFileLoaded { file_id, result } => {
                if let Some(viewer) = app.file_viewers.get_mut(&file_id) {
                    viewer.content = match result {
                        Ok(text) => {
                            // Detect if it's markdown based on file extension or content
                            let is_markdown = viewer.file_name.ends_with(".md")
                                || viewer.file_name.ends_with(".markdown")
                                || viewer.mime.contains("markdown");

                            if is_markdown {
                                // Initialize markdown cache
                                viewer.markdown_cache = Some(egui_commonmark::CommonMarkCache::default());
                                super::FileViewerContent::Markdown(text)
                            } else {
                                super::FileViewerContent::Text(text)
                            }
                        }
                        Err(err) => super::FileViewerContent::Error(err),
                    };
                }
            }
            AppMessage::MediaFileLoaded { file_id, result } => {
                if let Some(viewer) = app.file_viewers.get_mut(&file_id) {
                    viewer.content = match result {
                        Ok(bytes) => {
                            // Save to persistent cache directory
                            match super::get_video_cache_dir() {
                                Ok(cache_dir) => {
                                    let cache_path = cache_dir.join(format!("{}.mp4", file_id));
                                    match std::fs::write(&cache_path, bytes) {
                                        Ok(()) => {
                                            log::info!("Cached video to: {}", cache_path.display());

                                            // Create player with audio support
                                            let player_result = if let Some(audio_device) = &mut app.audio_device {
                                                egui_video::Player::new(app.ctx.as_ref().unwrap(), &cache_path.to_string_lossy().to_string())
                                                    .and_then(|mut player| {
                                                        // Set initial volume
                                                        player.options.set_audio_volume(app.video_volume);
                                                        player.with_audio(audio_device)
                                                    })
                                            } else {
                                                log::warn!("No audio device available, creating player without audio");
                                                egui_video::Player::new(app.ctx.as_ref().unwrap(), &cache_path.to_string_lossy().to_string())
                                            };

                                            match player_result {
                                                Ok(player) => super::FileViewerContent::Video(player),
                                                Err(err) => super::FileViewerContent::Error(format!("Failed to load video: {}", err)),
                                            }
                                        }
                                        Err(err) => super::FileViewerContent::Error(format!("Failed to cache video: {}", err)),
                                    }
                                }
                                Err(err) => super::FileViewerContent::Error(err),
                            }
                        }
                        Err(err) => super::FileViewerContent::Error(err),
                    };
                }
            }
            AppMessage::PdfFileLoaded { file_id, result } => {
                if let Some(viewer) = app.file_viewers.get_mut(&file_id) {
                    viewer.content = match result {
                        Ok(bytes) => super::FileViewerContent::Pdf(bytes),
                        Err(err) => super::FileViewerContent::Error(err),
                    };
                }
            }
            AppMessage::FileSaved { file_id: _, result } => {
                match result {
                    Ok(()) => {
                        app.info_banner = Some("File saved successfully".to_string());
                    }
                    Err(err) => {
                        app.info_banner = Some(format!("Failed to save file: {}", err));
                    }
                }
            }
        }
    }
}
