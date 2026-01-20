use std::collections::{HashMap, HashSet};

use eframe::egui;
use log::{error, info};

use crate::models::{
    BlockedPeerView, BlocklistEntryView, BlocklistSubscriptionView, ConversationView,
    DirectMessageView, FileResponse, PeerView, PostView, ReactionsResponse, SearchResponse, ThreadDetails,
    ThreadSummary,
};

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
    // Import
    ThreadImported(String),
    ImportError(String),
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
    UploadProgress {
        file_path: String,
        bytes_uploaded: u64,
        total_bytes: u64,
    },
    ReactionsLoaded {
        post_id: String,
        result: Result<ReactionsResponse, anyhow::Error>,
    },
    ReactionAdded {
        post_id: String,
        result: Result<(), anyhow::Error>,
    },
    ReactionRemoved {
        post_id: String,
        result: Result<(), anyhow::Error>,
    },
    ConversationsLoaded(Result<Vec<ConversationView>, anyhow::Error>),
    MessagesLoaded {
        peer_id: String,
        result: Result<Vec<DirectMessageView>, anyhow::Error>,
    },
    DmSent {
        to_peer_id: String,
        result: Result<DirectMessageView, anyhow::Error>,
    },
    BlockedPeersLoaded(Result<Vec<BlockedPeerView>, anyhow::Error>),
    PeerBlocked {
        peer_id: String,
        result: Result<BlockedPeerView, anyhow::Error>,
    },
    PeerUnblocked {
        peer_id: String,
        result: Result<(), anyhow::Error>,
    },
    BlocklistsLoaded(Result<Vec<BlocklistSubscriptionView>, anyhow::Error>),
    BlocklistSubscribed {
        blocklist_id: String,
        result: Result<BlocklistSubscriptionView, anyhow::Error>,
    },
    BlocklistUnsubscribed {
        blocklist_id: String,
        result: Result<(), anyhow::Error>,
    },
    BlocklistEntriesLoaded {
        blocklist_id: String,
        result: Result<Vec<BlocklistEntryView>, anyhow::Error>,
    },
    // IP Blocking messages
    IpBlocksLoaded(Result<Vec<crate::models::IpBlockView>, anyhow::Error>),
    IpBlockStatsLoaded(Result<crate::models::IpBlockStatsResponse, anyhow::Error>),
    IpBlockAdded {
        result: Result<(), anyhow::Error>,
    },
    IpBlockRemoved {
        block_id: i64,
        result: Result<(), anyhow::Error>,
    },
    IpBlocksImported {
        result: Result<(), anyhow::Error>,
    },
    IpBlocksExported {
        result: Result<String, anyhow::Error>,
    },
    IpBlocksCleared {
        result: Result<(), anyhow::Error>,
    },
    PeerIpBlocked {
        peer_id: String,
        blocked_ips: Vec<String>,
    },
    PeerIpBlockFailed {
        peer_id: String,
        error: String,
    },
    SearchCompleted {
        query: String,
        result: Result<SearchResponse, anyhow::Error>,
    },
    RecentPostsLoaded(Result<crate::models::RecentPostsResponse, anyhow::Error>),
    // Topic management messages
    TopicsLoaded(Result<Vec<String>, anyhow::Error>),
    TopicSubscribed {
        topic_id: String,
        result: Result<(), anyhow::Error>,
    },
    TopicUnsubscribed {
        topic_id: String,
        result: Result<(), anyhow::Error>,
    },
    // Theme management messages
    ThemeColorLoaded(Result<(u8, u8, u8), anyhow::Error>),
}

pub(super) fn process_messages(app: &mut GraphchanApp) {
    while let Ok(message) = app.rx.try_recv() {
        match message {
            AppMessage::ThreadsLoaded(result) => {
                app.threads_loading = false;
                app.is_refreshing = false;
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

                                // Check if this is a refresh (we already have details) or initial load
                                let is_refresh = state.details.is_some();

                                if is_refresh {
                                    // Smooth update - only add new posts
                                    let existing_post_ids: HashSet<String> = state.details.as_ref()
                                        .map(|d| d.posts.iter().map(|p| p.id.clone()).collect())
                                        .unwrap_or_default();

                                    // Find new posts
                                    let new_posts: Vec<_> = details.posts.iter()
                                        .filter(|p| !existing_post_ids.contains(&p.id))
                                        .collect();

                                    if !new_posts.is_empty() {
                                        // Add new posts to details
                                        if let Some(current_details) = &mut state.details {
                                            for new_post in &new_posts {
                                                current_details.posts.push((*new_post).clone());
                                            }
                                        }

                                        // Add new graph nodes for new posts only
                                        for post in &new_posts {
                                            if !state.graph_nodes.contains_key(&post.id) {
                                                state.graph_nodes.insert(
                                                    post.id.clone(),
                                                    super::state::GraphNode {
                                                        pos: egui::pos2(0.0, 0.0),
                                                        vel: egui::vec2(0.0, 0.0),
                                                        size: egui::vec2(300.0, 100.0),
                                                        dragging: false,
                                                        pinned: false,
                                                    }
                                                );
                                            }
                                        }

                                        // Update chronological/sugiyama nodes will happen on next render
                                        state.chronological_nodes.clear();
                                        state.sugiyama_nodes.clear();
                                    }

                                    // Update thread summary but preserve viewport
                                    state.summary = details.thread.clone();
                                } else {
                                    // Initial load - full reset
                                    state.summary = details.thread.clone();
                                    state.graph_nodes = build_initial_graph(&details.posts);
                                    state.chronological_nodes = HashMap::new();
                                    state.sugiyama_nodes = HashMap::new();
                                    state.sim_start_time = None;
                                    state.graph_zoom = 1.0;
                                    state.graph_offset = egui::vec2(0.0, 0.0);
                                    state.graph_dragging = false;
                                    state.repulsion_force = 500.0;
                                    state.sim_paused = false;
                                }

                                // Update global peer cache
                                for peer in &details.peers {
                                    app.peers.insert(peer.id.clone(), peer.clone());
                                }

                                // Update details (or set for first time)
                                if !is_refresh {
                                    state.details = Some(details);
                                }

                                state.error = None;

                                // Only clear these on initial load
                                if !is_refresh {
                                    state.attachments.clear();
                                    state.attachments_errors.clear();
                                    state.attachments_loading.clear();
                                    state.reply_to.clear();
                                    state.draft_attachments.clear();
                                }
                                
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
                        app.selected_topics.clear();
                        app.show_create_thread = false;
                        app.info_banner = Some("Thread created".into());
                        let summary = details.thread.clone();
                        let mut state = ThreadState {
                            summary: summary.clone(),
                            details: Some(details.clone()),
                            is_loading: false,
                            graph_nodes: build_initial_graph(&details.posts),
                            graph_zoom: 1.0,
                            time_bin_seconds: 60,
                            repulsion_force: 500.0,
                            ..Default::default()
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
            AppMessage::ThreadImported(thread_id) => {
                app.importer.importing = false;
                app.reddit_importer.importing = false;
                app.importer.error = None;
                app.reddit_importer.error = None;
                app.importer.url.clear();
                app.reddit_importer.url.clear();
                
                // Switch to the new thread
                app.view = ViewState::Thread(ThreadState::default());
                app.pending_thread_focus = Some(thread_id.clone());
                
                // Trigger load
                app.spawn_load_thread(&thread_id);
            }
            AppMessage::ImportError(err) => {
                app.importer.importing = false;
                app.reddit_importer.importing = false;

                if !app.importer.url.is_empty() {
                     app.importer.error = Some(err.clone());
                }
                if !app.reddit_importer.url.is_empty() {
                     app.reddit_importer.error = Some(err);
                }
            }
            AppMessage::ReactionsLoaded { post_id, result } => {
                if let ViewState::Thread(state) = &mut app.view {
                    state.reactions_loading.remove(&post_id);
                    match result {
                        Ok(reactions) => {
                            state.reactions.insert(post_id, reactions);
                        }
                        Err(err) => {
                            error!("Failed to load reactions for post {}: {}", post_id, err);
                        }
                    }
                }
            }
            AppMessage::ReactionAdded { post_id, result } => {
                match result {
                    Ok(()) => {
                        // Reload reactions for this post
                        app.spawn_load_reactions(&post_id);
                    }
                    Err(err) => {
                        error!("Failed to add reaction to post {}: {}", post_id, err);
                    }
                }
            }
            AppMessage::ReactionRemoved { post_id, result } => {
                match result {
                    Ok(()) => {
                        // Reload reactions for this post
                        app.spawn_load_reactions(&post_id);
                    }
                    Err(err) => {
                        error!("Failed to remove reaction from post {}: {}", post_id, err);
                    }
                }
            }
            AppMessage::ConversationsLoaded(result) => {
                app.dm_state.conversations_loading = false;
                match result {
                    Ok(conversations) => {
                        app.dm_state.conversations = conversations;
                        app.dm_state.conversations_error = None;
                    }
                    Err(err) => {
                        error!("Failed to load conversations: {}", err);
                        app.dm_state.conversations_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::MessagesLoaded { peer_id, result } => {
                if let ViewState::Conversation(ref mut state) = app.view {
                    if state.peer_id == peer_id {
                        state.messages_loading = false;
                        match result {
                            Ok(messages) => {
                                state.messages = messages;
                                state.messages_error = None;
                            }
                            Err(err) => {
                                error!("Failed to load messages for {}: {}", peer_id, err);
                                state.messages_error = Some(err.to_string());
                            }
                        }
                    }
                }
            }
            AppMessage::DmSent { to_peer_id, result } => {
                if let ViewState::Conversation(ref mut state) = app.view {
                    if state.peer_id == to_peer_id {
                        state.sending = false;
                        match result {
                            Ok(message) => {
                                state.messages.push(message);
                                state.new_message_body.clear();
                                state.send_error = None;
                                // Reload conversations to update unread counts
                                app.spawn_load_conversations();
                            }
                            Err(err) => {
                                error!("Failed to send DM to {}: {}", to_peer_id, err);
                                state.send_error = Some(err.to_string());
                            }
                        }
                    }
                }
            }
            AppMessage::BlockedPeersLoaded(result) => {
                app.blocking_state.blocked_peers_loading = false;
                match result {
                    Ok(peers) => {
                        app.blocking_state.blocked_peers = peers;
                        app.blocking_state.blocked_peers_error = None;
                    }
                    Err(err) => {
                        error!("Failed to load blocked peers: {}", err);
                        app.blocking_state.blocked_peers_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::PeerBlocked { peer_id, result } => {
                app.blocking_state.blocking_in_progress = false;
                match result {
                    Ok(blocked) => {
                        app.blocking_state.blocked_peers.push(blocked);
                        app.blocking_state.new_block_peer_id.clear();
                        app.blocking_state.new_block_reason.clear();
                        app.blocking_state.block_error = None;
                    }
                    Err(err) => {
                        error!("Failed to block peer {}: {}", peer_id, err);
                        app.blocking_state.block_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::PeerUnblocked { peer_id, result } => {
                match result {
                    Ok(_) => {
                        app.blocking_state.blocked_peers.retain(|p| p.peer_id != peer_id);
                    }
                    Err(err) => {
                        error!("Failed to unblock peer {}: {}", peer_id, err);
                        app.blocking_state.blocked_peers_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::BlocklistsLoaded(result) => {
                app.blocking_state.blocklists_loading = false;
                match result {
                    Ok(blocklists) => {
                        app.blocking_state.blocklists = blocklists;
                        app.blocking_state.blocklists_error = None;
                    }
                    Err(err) => {
                        error!("Failed to load blocklists: {}", err);
                        app.blocking_state.blocklists_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::BlocklistSubscribed { blocklist_id, result } => {
                app.blocking_state.subscribing_in_progress = false;
                match result {
                    Ok(subscription) => {
                        app.blocking_state.blocklists.push(subscription);
                        app.blocking_state.new_blocklist_id.clear();
                        app.blocking_state.new_blocklist_maintainer.clear();
                        app.blocking_state.new_blocklist_name.clear();
                        app.blocking_state.new_blocklist_description.clear();
                        app.blocking_state.new_blocklist_auto_apply = false;
                        app.blocking_state.subscribe_error = None;
                    }
                    Err(err) => {
                        error!("Failed to subscribe to blocklist {}: {}", blocklist_id, err);
                        app.blocking_state.subscribe_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::BlocklistUnsubscribed { blocklist_id, result } => {
                match result {
                    Ok(_) => {
                        app.blocking_state.blocklists.retain(|b| b.id != blocklist_id);
                    }
                    Err(err) => {
                        error!("Failed to unsubscribe from blocklist {}: {}", blocklist_id, err);
                        app.blocking_state.blocklists_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::BlocklistEntriesLoaded { blocklist_id, result } => {
                app.blocking_state.blocklist_entries_loading = false;
                match result {
                    Ok(entries) => {
                        app.blocking_state.blocklist_entries = entries;
                        app.blocking_state.blocklist_entries_error = None;
                    }
                    Err(err) => {
                        error!("Failed to load entries for blocklist {}: {}", blocklist_id, err);
                        app.blocking_state.blocklist_entries_error = Some(err.to_string());
                    }
                }
            }
            // IP Blocking handlers
            AppMessage::IpBlocksLoaded(result) => {
                app.blocking_state.ip_blocks_loading = false;
                match result {
                    Ok(blocks) => {
                        app.blocking_state.ip_blocks = blocks;
                        app.blocking_state.ip_blocks_error = None;
                    }
                    Err(err) => {
                        error!("Failed to load IP blocks: {}", err);
                        app.blocking_state.ip_blocks_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::IpBlockStatsLoaded(result) => {
                app.blocking_state.ip_block_stats_loading = false;
                match result {
                    Ok(stats) => {
                        app.blocking_state.ip_block_stats = Some(stats);
                    }
                    Err(err) => {
                        error!("Failed to load IP block stats: {}", err);
                    }
                }
            }
            AppMessage::IpBlockAdded { result } => {
                app.blocking_state.adding_ip_block = false;
                match result {
                    Ok(_) => {
                        app.blocking_state.new_ip_block.clear();
                        app.blocking_state.new_ip_block_reason.clear();
                        app.blocking_state.add_ip_block_error = None;
                        app.spawn_load_ip_blocks();
                        app.spawn_load_ip_block_stats();
                    }
                    Err(err) => {
                        error!("Failed to add IP block: {}", err);
                        app.blocking_state.add_ip_block_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::IpBlockRemoved { block_id, result } => {
                match result {
                    Ok(_) => {
                        app.blocking_state.ip_blocks.retain(|b| b.id != block_id);
                        app.spawn_load_ip_block_stats();
                    }
                    Err(err) => {
                        error!("Failed to remove IP block {}: {}", block_id, err);
                        app.blocking_state.ip_blocks_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::IpBlocksImported { result } => {
                app.blocking_state.importing_ips = false;
                match result {
                    Ok(_) => {
                        app.blocking_state.import_text.clear();
                        app.blocking_state.import_error = None;
                        app.spawn_load_ip_blocks();
                        app.spawn_load_ip_block_stats();
                    }
                    Err(err) => {
                        error!("Failed to import IP blocks: {}", err);
                        app.blocking_state.import_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::IpBlocksExported { result } => {
                match result {
                    Ok(export_text) => {
                        // Copy to clipboard or save to file
                        // For now, just log it
                        info!("IP blocks exported: {} bytes", export_text.len());
                        // TODO: Add clipboard or save dialog
                    }
                    Err(err) => {
                        error!("Failed to export IP blocks: {}", err);
                    }
                }
            }
            AppMessage::IpBlocksCleared { result } => {
                match result {
                    Ok(_) => {
                        app.blocking_state.ip_blocks.clear();
                        app.spawn_load_ip_block_stats();
                    }
                    Err(err) => {
                        error!("Failed to clear IP blocks: {}", err);
                        app.blocking_state.ip_blocks_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::PeerIpBlocked { peer_id, blocked_ips } => {
                info!("Blocked {} IP(s) for peer {}: {:?}", blocked_ips.len(), peer_id, blocked_ips);
                // Refresh IP blocks list
                app.spawn_load_ip_blocks();
                app.spawn_load_ip_block_stats();
                // Show success message
                app.info_banner = Some(format!("Blocked {} IP(s) for peer {}", blocked_ips.len(), peer_id));
            }
            AppMessage::PeerIpBlockFailed { peer_id, error } => {
                error!("Failed to block IPs for peer {}: {}", peer_id, error);
                app.info_banner = Some(format!("Failed to block IPs for peer {}: {}", peer_id, error));
            }
            AppMessage::SearchCompleted { query, result } => {
                if let ViewState::SearchResults(state) = &mut app.view {
                    if state.query == query {
                        state.is_loading = false;
                        match result {
                            Ok(response) => {
                                state.results = response.results;
                                state.error = None;
                            }
                            Err(err) => {
                                state.error = Some(err.to_string());
                                state.results.clear();
                            }
                        }
                    }
                }
            }
            AppMessage::RecentPostsLoaded(result) => {
                app.recent_posts_loading = false;
                match result {
                    Ok(response) => {
                        app.recent_posts = response.posts;
                        app.recent_posts_error = None;
                    }
                    Err(err) => {
                        error!("Failed to load recent posts: {}", err);
                        app.recent_posts_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::UploadProgress {
                file_path: _,
                bytes_uploaded: _,
                total_bytes: _,
            } => {
                // TODO: Display upload progress in UI
            }
            AppMessage::TopicsLoaded(result) => {
                app.topics_loading = false;
                match result {
                    Ok(topics) => {
                        app.subscribed_topics = topics;
                        app.topics_error = None;
                    }
                    Err(err) => {
                        error!("Failed to load topics: {}", err);
                        app.topics_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::TopicSubscribed { topic_id, result } => {
                match result {
                    Ok(()) => {
                        if !app.subscribed_topics.contains(&topic_id) {
                            app.subscribed_topics.push(topic_id.clone());
                        }
                        app.new_topic_input.clear();
                        info!("Successfully subscribed to topic: {}", topic_id);
                    }
                    Err(err) => {
                        error!("Failed to subscribe to topic {}: {}", topic_id, err);
                        app.topics_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::TopicUnsubscribed { topic_id, result } => {
                match result {
                    Ok(()) => {
                        app.subscribed_topics.retain(|t| t != &topic_id);
                        app.selected_topics.remove(&topic_id);
                        info!("Successfully unsubscribed from topic: {}", topic_id);
                    }
                    Err(err) => {
                        error!("Failed to unsubscribe from topic {}: {}", topic_id, err);
                        app.topics_error = Some(err.to_string());
                    }
                }
            }
            AppMessage::ThemeColorLoaded(result) => {
                match result {
                    Ok((r, g, b)) => {
                        app.primary_color = egui::Color32::from_rgb(r, g, b);
                        app.theme_dirty = true;
                        info!("Theme color loaded: RGB({}, {}, {})", r, g, b);
                    }
                    Err(err) => {
                        error!("Failed to load theme color: {}", err);
                        // Use default color on error
                        app.primary_color = egui::Color32::from_rgb(64, 128, 255);
                        app.theme_dirty = true;
                    }
                }
            }
        }
    }
}
