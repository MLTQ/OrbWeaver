use std::collections::HashMap;

use crate::models::{
    BlockedPeerView, BlocklistEntryView, BlocklistSubscriptionView, ConversationView,
    DirectMessageView, FileResponse, PeerView, PostView, ReactionsResponse, SearchResponse, ThreadDetails,
    ThreadSummary,
};

use super::state::LoadedImage;
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
    // Source refresh (imported thread update from 4chan/Reddit)
    ThreadSourceRefreshed {
        thread_id: String,
        result: Result<ThreadDetails, anyhow::Error>,
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
    PeerBlocksExported {
        result: Result<String, anyhow::Error>,
    },
    PeerBlocksImported {
        result: Result<(), anyhow::Error>,
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

/// Dispatch incoming messages to domain-specific handler methods on GraphchanApp.
pub(super) fn process_messages(app: &mut GraphchanApp) {
    while let Ok(message) = app.rx.try_recv() {
        match message {
            // Thread/Post handlers (handlers_threads.rs)
            AppMessage::ThreadsLoaded(result) => app.handle_threads_loaded(result),
            AppMessage::ThreadLoaded { thread_id, result } => app.handle_thread_loaded(thread_id, result),
            AppMessage::ThreadCreated(result) => app.handle_thread_created(result),
            AppMessage::PostCreated { thread_id, result } => app.handle_post_created(thread_id, result),
            AppMessage::PostAttachmentsLoaded { thread_id, post_id, result } => {
                app.handle_post_attachments_loaded(thread_id, post_id, result)
            }

            // File handlers (handlers_files.rs)
            AppMessage::ImageLoaded { file_id, result } => app.handle_image_loaded(file_id, result),
            AppMessage::TextFileLoaded { file_id, result } => app.handle_text_file_loaded(file_id, result),
            AppMessage::MediaFileLoaded { file_id, result } => app.handle_media_file_loaded(file_id, result),
            AppMessage::PdfFileLoaded { file_id, result } => app.handle_pdf_file_loaded(file_id, result),
            AppMessage::FileSaved { file_id: _, result } => app.handle_file_saved(result),

            // Import handlers (handlers_misc.rs)
            AppMessage::ImportFinished(result) => app.handle_import_finished(result),
            AppMessage::ThreadImported(thread_id) => app.handle_thread_imported(thread_id),
            AppMessage::ImportError(err) => app.handle_import_error(err),
            AppMessage::ThreadSourceRefreshed { thread_id, result } => app.handle_thread_source_refreshed(thread_id, result),

            // Identity/Peer handlers (handlers_misc.rs)
            AppMessage::IdentityLoaded(result) => app.handle_identity_loaded(result),
            AppMessage::PeersLoaded(result) => app.handle_peers_loaded(result),
            AppMessage::AvatarUploaded(result) => app.handle_avatar_uploaded(result),
            AppMessage::ProfileUpdated(result) => app.handle_profile_updated(result),
            AppMessage::PeerAdded(result) => app.handle_peer_added(result),
            AppMessage::ThreadFilesSelected(files) => app.handle_thread_files_selected(files),

            // Reaction handlers (handlers_misc.rs)
            AppMessage::ReactionsLoaded { post_id, result } => app.handle_reactions_loaded(post_id, result),
            AppMessage::ReactionAdded { post_id, result } => app.handle_reaction_added(post_id, result),
            AppMessage::ReactionRemoved { post_id, result } => app.handle_reaction_removed(post_id, result),

            // DM handlers (handlers_misc.rs)
            AppMessage::ConversationsLoaded(result) => app.handle_conversations_loaded(result),
            AppMessage::MessagesLoaded { peer_id, result } => app.handle_messages_loaded(peer_id, result),
            AppMessage::DmSent { to_peer_id, result } => app.handle_dm_sent(to_peer_id, result),

            // Blocking handlers (handlers_blocking.rs)
            AppMessage::BlockedPeersLoaded(result) => app.handle_blocked_peers_loaded(result),
            AppMessage::PeerBlocked { peer_id, result } => app.handle_peer_blocked(peer_id, result),
            AppMessage::PeerUnblocked { peer_id, result } => app.handle_peer_unblocked(peer_id, result),
            AppMessage::BlocklistsLoaded(result) => app.handle_blocklists_loaded(result),
            AppMessage::BlocklistSubscribed { blocklist_id, result } => app.handle_blocklist_subscribed(blocklist_id, result),
            AppMessage::BlocklistUnsubscribed { blocklist_id, result } => app.handle_blocklist_unsubscribed(blocklist_id, result),
            AppMessage::BlocklistEntriesLoaded { blocklist_id, result } => app.handle_blocklist_entries_loaded(blocklist_id, result),

            // IP Blocking handlers (handlers_blocking.rs)
            AppMessage::IpBlocksLoaded(result) => app.handle_ip_blocks_loaded(result),
            AppMessage::IpBlockStatsLoaded(result) => app.handle_ip_block_stats_loaded(result),
            AppMessage::IpBlockAdded { result } => app.handle_ip_block_added(result),
            AppMessage::IpBlockRemoved { block_id, result } => app.handle_ip_block_removed(block_id, result),
            AppMessage::IpBlocksImported { result } => app.handle_ip_blocks_imported(result),
            AppMessage::IpBlocksExported { result } => app.handle_ip_blocks_exported(result),
            AppMessage::PeerBlocksExported { result } => app.handle_peer_blocks_exported(result),
            AppMessage::PeerBlocksImported { result } => app.handle_peer_blocks_imported(result),
            AppMessage::IpBlocksCleared { result } => app.handle_ip_blocks_cleared(result),
            AppMessage::PeerIpBlocked { peer_id, blocked_ips } => app.handle_peer_ip_blocked(peer_id, blocked_ips),
            AppMessage::PeerIpBlockFailed { peer_id, error } => app.handle_peer_ip_block_failed(peer_id, error),

            // Search/Recent/Topics/Theme handlers (handlers_misc.rs)
            AppMessage::SearchCompleted { query, result } => app.handle_search_completed(query, result),
            AppMessage::RecentPostsLoaded(result) => app.handle_recent_posts_loaded(result),
            AppMessage::UploadProgress { .. } => {
                // TODO: Display upload progress in UI
            }
            AppMessage::TopicsLoaded(result) => app.handle_topics_loaded(result),
            AppMessage::TopicSubscribed { topic_id, result } => app.handle_topic_subscribed(topic_id, result),
            AppMessage::TopicUnsubscribed { topic_id, result } => app.handle_topic_unsubscribed(topic_id, result),
            AppMessage::ThemeColorLoaded(result) => app.handle_theme_color_loaded(result),
        }
    }
}
