use serde::{Deserialize, Serialize};

// Reuse models from graphchan API responses

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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerView {
    pub id: String,
    pub alias: Option<String>,
    pub username: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CreatePostInput {
    pub thread_id: String,
    #[serde(default)]
    pub author_peer_id: Option<String>,
    pub body: String,
    #[serde(default)]
    pub parent_post_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPostView {
    pub post: PostView,
    pub thread_title: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentPostsResponse {
    pub posts: Vec<RecentPostView>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostResponse {
    pub post: PostView,
}
