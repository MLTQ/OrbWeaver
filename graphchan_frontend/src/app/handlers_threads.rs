use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::models::{FileResponse, PostView, ThreadDetails, ThreadSummary};

use super::state::{CreateThreadState, GraphNode, ThreadState, ViewState};
use super::ui::graph::build_initial_graph;
use super::GraphchanApp;

impl GraphchanApp {
    pub(super) fn handle_threads_loaded(&mut self, result: Result<Vec<ThreadSummary>, anyhow::Error>) {
        self.threads_loading = false;
        self.is_refreshing = false;
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

    pub(super) fn handle_thread_loaded(&mut self, thread_id: String, result: Result<ThreadDetails, anyhow::Error>) {
        if let ViewState::Thread(state) = &mut self.view {
            if state.summary.id == thread_id {
                state.is_loading = false;
                match result {
                    Ok(details) => {
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
                                            GraphNode {
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
                            state.sim_running = true;
                            state.graph_zoom = 1.0;
                            state.graph_offset = egui::vec2(0.0, 0.0);
                            state.graph_dragging = false;
                            state.repulsion_force = 500.0;
                        }

                        // Update global peer cache
                        for peer in &details.peers {
                            self.peers.insert(peer.id.clone(), peer.clone());
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
                                                    crate::app::resolve_blob_url(&self.base_url_input, blob_id)
                                                } else {
                                                    crate::app::resolve_file_url(&self.base_url_input, &file.id)
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
                            self.spawn_load_image(&file_id, &url);
                        }
                    }
                    Err(err) => {
                        state.error = Some(err.to_string());
                    }
                }
            }
        }
    }

    pub(super) fn handle_thread_created(&mut self, result: Result<ThreadDetails, anyhow::Error>) {
        self.create_thread.submitting = false;
        match result {
            Ok(details) => {
                self.create_thread = CreateThreadState::default();
                self.selected_topics.clear();
                self.show_create_thread = false;
                self.info_banner = Some("Thread created".into());
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
                self.view = ViewState::Thread(state);
                // Attachments are already populated in ThreadDetails
                self.spawn_load_threads();
            }
            Err(err) => {
                self.create_thread.error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_post_created(&mut self, thread_id: String, result: Result<PostView, anyhow::Error>) {
        if let ViewState::Thread(state) = &mut self.view {
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
                        self.info_banner = Some("Post published".into());
                        let mut downloads = Vec::new();
                        if !post.files.is_empty() {
                            state.attachments.insert(post.id.clone(), post.files.clone());
                            // Trigger image downloads
                            for file in &post.files {
                                if let Some(mime) = &file.mime {
                                    if mime.starts_with("image/") {
                                        let url = if let Some(blob_id) = &file.blob_id {
                                            crate::app::resolve_blob_url(&self.base_url_input, blob_id)
                                        } else {
                                            crate::app::resolve_file_url(&self.base_url_input, &file.id)
                                        };
                                        downloads.push((file.id.clone(), url));
                                    }
                                }
                            }
                        }

                        for (file_id, url) in downloads {
                            self.spawn_load_image(&file_id, &url);
                        }
                    }
                    Err(err) => {
                        state.new_post_error = Some(err.to_string());
                    }
                }
            }
        }
    }

    pub(super) fn handle_post_attachments_loaded(
        &mut self,
        thread_id: String,
        post_id: String,
        result: Result<Vec<FileResponse>, anyhow::Error>,
    ) {
        let mut downloads: Vec<(String, String)> = Vec::new();
        let mut base_url: Option<String> = None;

        if let ViewState::Thread(state) = &mut self.view {
            if state.summary.id != thread_id {
                return;
            }
            state.attachments_loading.remove(&post_id);
            match result {
                Ok(files) => {
                    for file in &files {
                        if file.present {
                            if let Some(mime) = &file.mime {
                                let already_have = self.image_loading.contains(&file.id)
                                    || self.image_textures.contains_key(&file.id)
                                    || self.image_errors.contains_key(&file.id);
                                if mime.starts_with("image/") && !already_have {
                                    let base_ref = base_url.get_or_insert_with(|| {
                                        self.api.base_url().to_string()
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
                    base_url.get_or_insert_with(|| self.api.base_url().to_string());
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
                self.spawn_download_image(&file_id, &url);
            }
        }
    }
}
