use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub creator_peer_id: Option<String>,
    pub created_at: String,
    #[serde(default)]
    pub pinned: bool,
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
