use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRecord {
    pub id: String,
    pub alias: Option<String>,
    pub friendcode: Option<String>,
    pub iroh_peer_id: Option<String>,
    pub gpg_fingerprint: Option<String>,
    pub last_seen: Option<String>,
    pub trust_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub id: String,
    pub title: String,
    pub creator_peer_id: Option<String>,
    pub created_at: String,
    pub pinned: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRecord {
    pub id: String,
    pub thread_id: String,
    pub author_peer_id: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostEdge {
    pub parent_id: String,
    pub child_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRecord {
    pub id: String,
    pub post_id: String,
    pub path: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub blob_id: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
}
