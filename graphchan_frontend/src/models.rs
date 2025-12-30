use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub creator_peer_id: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub pinned: bool,
    #[serde(default = "default_sync_status")]
    pub sync_status: String,
    #[serde(default)]
    pub first_image_file: Option<FileResponse>,
}

fn default_sync_status() -> String {
    "downloaded".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadDetails {
    pub thread: ThreadSummary,
    #[serde(default)]
    pub posts: Vec<PostView>,
    #[serde(default)]
    pub peers: Vec<PeerView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostView {
    pub id: String,
    pub thread_id: String,
    #[serde(default)]
    pub author_peer_id: Option<String>,
    #[serde(default)]
    pub author_short_friendcode: Option<String>,
    pub body: String,
    pub created_at: String,
    #[serde(default)]
    pub updated_at: Option<String>,
    #[serde(default)]
    pub parent_post_ids: Vec<String>,
    #[serde(default)]
    pub files: Vec<FileResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreateThreadInput {
    pub title: String,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub creator_peer_id: Option<String>,
    #[serde(default)]
    pub pinned: Option<bool>,
    /// Optional timestamp for imported threads. If None, backend uses current time.
    #[serde(default)]
    pub created_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreatePostInput {
    pub thread_id: String,
    #[serde(default)]
    pub author_peer_id: Option<String>,
    pub body: String,
    #[serde(default)]
    pub parent_post_ids: Vec<String>,
    /// Optional timestamp for imported posts. If None, backend uses current time.
    #[serde(default)]
    pub created_at: Option<String>,
    /// Whether to rebroadcast this thread to peers (Host mode)
    #[serde(default = "default_rebroadcast")]
    pub rebroadcast: bool,
}

fn default_rebroadcast() -> bool {
    true // Default to Host mode
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostResponse {
    pub post: PostView,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResponse {
    pub id: String,
    pub post_id: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub blob_id: Option<String>,
    pub ticket: Option<String>,
    pub path: String,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub present: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FileListEntry(pub FileResponse);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerView {
    pub id: String,
    pub alias: Option<String>,
    pub username: Option<String>,
    pub bio: Option<String>,
    pub friendcode: Option<String>,
    #[serde(default)]
    pub short_friendcode: Option<String>,
    pub iroh_peer_id: Option<String>,
    pub gpg_fingerprint: Option<String>,
    pub last_seen: Option<String>,
    pub avatar_file_id: Option<String>,
    pub trust_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProfileRequest {
    pub username: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AddPeerRequest {
    pub friendcode: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionsResponse {
    pub reactions: Vec<ReactionView>,
    pub counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionView {
    pub emoji: String,
    pub reactor_peer_id: String,
    pub created_at: String,
}

// Direct Message models

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationView {
    pub id: String,
    pub peer_id: String,
    pub peer_username: Option<String>,
    pub peer_alias: Option<String>,
    pub last_message_at: Option<String>,
    pub last_message_preview: Option<String>,
    pub unread_count: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessageView {
    pub id: String,
    pub conversation_id: String,
    pub from_peer_id: String,
    pub to_peer_id: String,
    pub body: String,
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendDmRequest {
    pub to_peer_id: String,
    pub body: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnreadCountResponse {
    pub count: usize,
}

// Blocking and Moderation models

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedPeerView {
    pub peer_id: String,
    pub peer_username: Option<String>,
    pub peer_alias: Option<String>,
    pub reason: Option<String>,
    pub blocked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistSubscriptionView {
    pub id: String,
    pub maintainer_peer_id: String,
    pub name: String,
    pub description: Option<String>,
    pub auto_apply: bool,
    pub last_synced_at: Option<String>,
    pub entry_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistEntryView {
    pub peer_id: String,
    pub peer_username: Option<String>,
    pub peer_alias: Option<String>,
    pub reason: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockPeerRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeBlocklistRequest {
    pub maintainer_peer_id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default = "default_auto_apply")]
    pub auto_apply: bool,
}

fn default_auto_apply() -> bool {
    true
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultView {
    pub result_type: String,
    pub post: PostView,
    pub file: Option<FileResponse>,
    pub thread_id: String,
    pub thread_title: String,
    pub bm25_score: f64,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResponse {
    pub results: Vec<SearchResultView>,
    pub query: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPostView {
    pub post: PostView,
    pub thread_title: String,
    pub files: Vec<FileResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPostsResponse {
    pub posts: Vec<RecentPostView>,
}
