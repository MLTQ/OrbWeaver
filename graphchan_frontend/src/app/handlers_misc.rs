use eframe::egui;
use log::{error, info};

use crate::models::{
    ConversationView, DirectMessageView, PeerView, ReactionsResponse, SearchResponse,
};

use super::state::{ThreadState, ViewState};
use super::GraphchanApp;

impl GraphchanApp {
    // Import handlers

    pub(super) fn handle_import_finished(&mut self, result: Result<String, anyhow::Error>) {
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
                self.importer.error = Some(format!("{err:#}"));
            }
        }
    }

    pub(super) fn handle_thread_imported(&mut self, thread_id: String) {
        self.importer.importing = false;
        self.importer.error = None;
        self.importer.url.clear();
        self.importer.open = false;

        // Switch to the new thread
        self.view = ViewState::Thread(ThreadState::default());
        self.pending_thread_focus = Some(thread_id.clone());

        // Trigger load
        self.spawn_load_thread(&thread_id);
    }

    pub(super) fn handle_import_error(&mut self, err: String) {
        self.importer.importing = false;
        self.importer.error = Some(err);
    }

    pub(super) fn handle_thread_source_refreshed(&mut self, thread_id: String, result: Result<crate::models::ThreadDetails, anyhow::Error>) {
        if let ViewState::Thread(ref mut state) = self.view {
            if state.summary.id == thread_id {
                state.refreshing_source = false;
                match result {
                    Ok(details) => {
                        state.summary = details.thread.clone();
                        state.details = Some(details);
                        state.refresh_error = None;
                        self.info_banner = Some("Thread refreshed from source".into());
                    }
                    Err(err) => {
                        state.refresh_error = Some(format!("{err:#}"));
                    }
                }
            }
        }
    }

    // Identity/Peer handlers

    pub(super) fn handle_identity_loaded(&mut self, result: Result<PeerView, anyhow::Error>) {
        let state = &mut self.identity_state;
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

    pub(super) fn handle_peers_loaded(&mut self, result: Result<Vec<PeerView>, anyhow::Error>) {
        match result {
            Ok(peers) => {
                for peer in peers {
                    self.peers.insert(peer.id.clone(), peer);
                }
            }
            Err(err) => {
                error!("Failed to load peers: {err}");
            }
        }
    }

    pub(super) fn handle_avatar_uploaded(&mut self, result: Result<(), anyhow::Error>) {
        let state = &mut self.identity_state;
        state.uploading = false;
        match result {
            Ok(()) => {
                state.avatar_path = None;
                // Reload identity to get new avatar ID
                self.spawn_load_identity();
            }
            Err(err) => {
                state.error = Some(format!("Failed to upload avatar: {err}"));
            }
        }
    }

    pub(super) fn handle_profile_updated(&mut self, result: Result<(), anyhow::Error>) {
        let state = &mut self.identity_state;
        state.uploading = false;
        match result {
            Ok(()) => {
                self.spawn_load_identity();
            }
            Err(err) => {
                state.error = Some(format!("Failed to update profile: {err}"));
            }
        }
    }

    pub(super) fn handle_peer_added(&mut self, result: Result<PeerView, anyhow::Error>) {
        let state = &mut self.identity_state;
        state.adding_peer = false;
        match result {
            Ok(peer) => {
                self.peers.insert(peer.id.clone(), peer.clone());
                self.info_banner = Some(format!("Added peer: {}", peer.username.as_deref().unwrap_or("Unknown")));
                state.friendcode_input.clear();
                state.error = None;
            }
            Err(err) => {
                state.error = Some(format!("Failed to add peer: {err}"));
            }
        }
    }

    pub(super) fn handle_thread_files_selected(&mut self, files: Vec<std::path::PathBuf>) {
        self.create_thread.files.extend(files);
    }

    // Reaction handlers

    pub(super) fn handle_reactions_loaded(&mut self, post_id: String, result: Result<ReactionsResponse, anyhow::Error>) {
        if let ViewState::Thread(state) = &mut self.view {
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

    pub(super) fn handle_reaction_added(&mut self, post_id: String, result: Result<(), anyhow::Error>) {
        match result {
            Ok(()) => {
                // Reload reactions for this post
                self.spawn_load_reactions(&post_id);
            }
            Err(err) => {
                error!("Failed to add reaction to post {}: {}", post_id, err);
            }
        }
    }

    pub(super) fn handle_reaction_removed(&mut self, post_id: String, result: Result<(), anyhow::Error>) {
        match result {
            Ok(()) => {
                // Reload reactions for this post
                self.spawn_load_reactions(&post_id);
            }
            Err(err) => {
                error!("Failed to remove reaction from post {}: {}", post_id, err);
            }
        }
    }

    // DM handlers

    pub(super) fn handle_conversations_loaded(&mut self, result: Result<Vec<ConversationView>, anyhow::Error>) {
        self.dm_state.conversations_loading = false;
        match result {
            Ok(conversations) => {
                self.dm_state.conversations = conversations;
                self.dm_state.conversations_error = None;
            }
            Err(err) => {
                error!("Failed to load conversations: {}", err);
                self.dm_state.conversations_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_messages_loaded(&mut self, peer_id: String, result: Result<Vec<DirectMessageView>, anyhow::Error>) {
        if let ViewState::Conversation(ref mut state) = self.view {
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

    pub(super) fn handle_dm_sent(&mut self, to_peer_id: String, result: Result<DirectMessageView, anyhow::Error>) {
        if let ViewState::Conversation(ref mut state) = self.view {
            if state.peer_id == to_peer_id {
                state.sending = false;
                match result {
                    Ok(message) => {
                        state.messages.push(message);
                        state.new_message_body.clear();
                        state.send_error = None;
                        // Reload conversations to update unread counts
                        self.spawn_load_conversations();
                    }
                    Err(err) => {
                        error!("Failed to send DM to {}: {}", to_peer_id, err);
                        state.send_error = Some(err.to_string());
                    }
                }
            }
        }
    }

    // Search handler

    pub(super) fn handle_search_completed(&mut self, query: String, result: Result<SearchResponse, anyhow::Error>) {
        if let ViewState::SearchResults(state) = &mut self.view {
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

    // Recent posts handler

    pub(super) fn handle_recent_posts_loaded(&mut self, result: Result<crate::models::RecentPostsResponse, anyhow::Error>) {
        self.recent_posts_loading = false;
        match result {
            Ok(response) => {
                self.recent_posts = response.posts;
                self.recent_posts_error = None;
            }
            Err(err) => {
                error!("Failed to load recent posts: {}", err);
                self.recent_posts_error = Some(err.to_string());
            }
        }
    }

    // Topic handlers

    pub(super) fn handle_topics_loaded(&mut self, result: Result<Vec<String>, anyhow::Error>) {
        self.topics_loading = false;
        match result {
            Ok(topics) => {
                self.subscribed_topics = topics;
                self.topics_error = None;
            }
            Err(err) => {
                error!("Failed to load topics: {}", err);
                self.topics_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_topic_subscribed(&mut self, topic_id: String, result: Result<(), anyhow::Error>) {
        match result {
            Ok(()) => {
                if !self.subscribed_topics.contains(&topic_id) {
                    self.subscribed_topics.push(topic_id.clone());
                }
                self.new_topic_input.clear();
                info!("Successfully subscribed to topic: {}", topic_id);
            }
            Err(err) => {
                error!("Failed to subscribe to topic {}: {}", topic_id, err);
                self.topics_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_topic_unsubscribed(&mut self, topic_id: String, result: Result<(), anyhow::Error>) {
        match result {
            Ok(()) => {
                self.subscribed_topics.retain(|t| t != &topic_id);
                self.selected_topics.remove(&topic_id);
                info!("Successfully unsubscribed from topic: {}", topic_id);
            }
            Err(err) => {
                error!("Failed to unsubscribe from topic {}: {}", topic_id, err);
                self.topics_error = Some(err.to_string());
            }
        }
    }

    // Theme handler

    pub(super) fn handle_theme_color_loaded(&mut self, result: Result<(u8, u8, u8), anyhow::Error>) {
        match result {
            Ok((r, g, b)) => {
                self.primary_color = egui::Color32::from_rgb(r, g, b);
                self.theme_dirty = true;
                info!("Theme color loaded: RGB({}, {}, {})", r, g, b);
            }
            Err(err) => {
                error!("Failed to load theme color: {}", err);
                // Use default color on error
                self.primary_color = egui::Color32::from_rgb(64, 128, 255);
                self.theme_dirty = true;
            }
        }
    }
}
