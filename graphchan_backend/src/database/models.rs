use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerRecord {
    pub id: String,
    pub alias: Option<String>,
    pub username: Option<String>,
    pub bio: Option<String>,
    pub friendcode: Option<String>,
    pub iroh_peer_id: Option<String>,
    pub gpg_fingerprint: Option<String>,
    pub x25519_pubkey: Option<String>,
    pub last_seen: Option<String>,
    pub avatar_file_id: Option<String>,
    pub trust_state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub id: String,
    pub title: String,
    pub creator_peer_id: Option<String>,
    pub created_at: String,
    pub pinned: bool,
    pub thread_hash: Option<String>,
    pub visibility: String,  // 'social' or 'private'
    pub topic_secret: Option<String>,  // base64-encoded 32-byte secret for private threads
    pub sync_status: String,  // 'announced', 'downloading', 'downloaded', 'failed'
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostRecord {
    pub id: String,
    pub thread_id: String,
    pub author_peer_id: Option<String>,
    /// Full legacy friendcode (v2 format with multiaddrs) for IP extraction
    pub author_friendcode: Option<String>,
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
    pub ticket: Option<String>,
    pub download_status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReactionRecord {
    pub post_id: String,
    pub reactor_peer_id: String,
    pub emoji: String,
    pub signature: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadMemberKey {
    pub thread_id: String,
    pub member_peer_id: String,
    pub wrapped_key_ciphertext: Vec<u8>,
    pub wrapped_key_nonce: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessageRecord {
    pub id: String,
    pub conversation_id: String,
    pub from_peer_id: String,
    pub to_peer_id: String,
    pub encrypted_body: Vec<u8>,
    pub nonce: Vec<u8>,
    pub created_at: String,
    pub read_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationRecord {
    pub id: String,
    pub peer_id: String,
    pub last_message_at: Option<String>,
    pub last_message_preview: Option<String>,
    pub unread_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedPeerRecord {
    pub peer_id: String,
    pub reason: Option<String>,
    pub blocked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistSubscriptionRecord {
    pub id: String,
    pub maintainer_peer_id: String,
    pub name: String,
    pub description: Option<String>,
    pub auto_apply: bool,
    pub last_synced_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistEntryRecord {
    pub blocklist_id: String,
    pub peer_id: String,
    pub reason: Option<String>,
    pub added_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedPostRecord {
    pub id: String,
    pub thread_id: String,
    pub author_peer_id: String,
    pub parent_post_ids: String, // JSON array
    pub known_child_ids: Option<String>, // JSON array
    pub redaction_reason: String,
    pub discovered_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SearchResultType {
    Post,
    File,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchResultRecord {
    pub result_type: SearchResultType,
    pub post: PostRecord,
    pub file: Option<FileRecord>,
    pub bm25_score: f64,
    pub thread_title: String,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerIpRecord {
    pub peer_id: String,
    pub ip_address: String,
    pub last_seen: i64,  // Unix timestamp
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpBlockRecord {
    pub id: i64,
    pub ip_or_range: String,
    pub block_type: String,  // "exact" or "range"
    pub blocked_at: i64,     // Unix timestamp
    pub reason: Option<String>,
    pub active: bool,
    pub hit_count: i64,
}
