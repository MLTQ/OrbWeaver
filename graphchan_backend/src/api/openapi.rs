//! OpenAPI / JSON Schema description of the REST surface.
//!
//! Kept deliberately self-contained: utoipa requires every type referenced
//! from a `path = ...` to implement `ToSchema`, which would otherwise spread
//! `utoipa` annotations across the entire codebase. Instead, we declare local
//! schema-only mirror types here and reference them from path annotations.
//! When response types stabilize they can grow proper `#[derive(ToSchema)]`
//! attributes upstream and this module shrinks.
//!
//! Coverage: the high-traffic agent-facing endpoints (threads, posts, peers,
//! search, events). Less-used routes (DMs, blocking, topics, settings) are
//! grouped under tags but currently document only the path + tag, so an
//! integrator can still discover them; bodies/responses get a generic JSON
//! object schema.
//!
//! Served at `/openapi.json` (public, no auth required) so any client can
//! fetch the spec to generate a typed client.

use serde::{Deserialize, Serialize};
use utoipa::{OpenApi, ToSchema};

// --- Mirror types ----------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ThreadSummary {
    pub id: String,
    pub title: String,
    pub creator_peer_id: Option<String>,
    pub created_at: String,
    pub pinned: bool,
    pub visibility: String,
    pub sync_status: String,
    pub topics: Vec<String>,
    pub source_url: Option<String>,
    pub source_platform: Option<String>,
    pub last_refreshed_at: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostView {
    pub id: String,
    pub thread_id: String,
    pub author_peer_id: Option<String>,
    pub author_friendcode: Option<String>,
    pub body: String,
    pub created_at: String,
    pub updated_at: Option<String>,
    pub parent_post_ids: Vec<String>,
    pub files: Vec<FileView>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct FileView {
    pub id: String,
    pub post_id: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub blob_id: Option<String>,
    pub ticket: Option<String>,
    pub path: String,
    pub download_status: Option<String>,
    pub present: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ThreadDetails {
    pub thread: ThreadSummary,
    pub posts: Vec<PostView>,
    pub peers: Vec<PeerView>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PeerView {
    pub id: String,
    pub alias: Option<String>,
    pub username: Option<String>,
    pub bio: Option<String>,
    pub friendcode: Option<String>,
    pub iroh_peer_id: Option<String>,
    pub gpg_fingerprint: Option<String>,
    pub trust_state: String,
    pub avatar_file_id: Option<String>,
    pub last_seen: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub api_port: u16,
    pub identity: IdentityInfo,
    pub network: NetworkInfo,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct IdentityInfo {
    pub gpg_fingerprint: String,
    pub iroh_peer_id: String,
    pub friendcode: String,
    pub short_friendcode: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct NetworkInfo {
    pub peer_id: String,
    pub addresses: Vec<String>,
    /// One of "checking", "connected", or "unreachable".
    pub dht_status: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct CreatePostInput {
    pub body: String,
    #[serde(default)]
    pub parent_post_ids: Vec<String>,
    #[serde(default)]
    pub rebroadcast: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct PostResponse {
    pub post: PostView,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct AddPeerRequest {
    pub friendcode: String,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
pub struct ErrorResponse {
    pub message: String,
}

// --- Path annotations ------------------------------------------------------
//
// These functions are markers that exist solely so utoipa's `paths(...)` macro
// can reference them. The actual handlers are wired up in api/mod.rs and use
// stricter (non-ToSchema) types — duplicating the signature here lets us
// document them without forcing utoipa annotations on every internal type.

#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses((status = 200, body = HealthResponse))
)]
fn _doc_health() {}

#[utoipa::path(
    get,
    path = "/events",
    tag = "system",
    description = "Server-Sent Events stream of live AppEvent payloads. Event names: post_added, thread_announced, file_announced, file_downloaded, profile_updated, reaction_updated, dm_received, lagged.",
    responses((status = 200, content_type = "text/event-stream"))
)]
fn _doc_events() {}

#[utoipa::path(
    get,
    path = "/threads",
    tag = "threads",
    params(("limit" = Option<usize>, Query, description = "Max threads to return (default 50, capped 200)")),
    responses((status = 200, body = Vec<ThreadSummary>))
)]
fn _doc_list_threads() {}

#[utoipa::path(
    get,
    path = "/threads/{id}",
    tag = "threads",
    params(("id" = String, Path, description = "Thread UUID")),
    responses(
        (status = 200, body = ThreadDetails),
        (status = 404, body = ErrorResponse)
    )
)]
fn _doc_get_thread() {}

#[utoipa::path(
    post,
    path = "/threads/{id}/posts",
    tag = "threads",
    params(("id" = String, Path, description = "Thread UUID")),
    request_body = CreatePostInput,
    responses(
        (status = 201, body = PostResponse),
        (status = 404, body = ErrorResponse)
    )
)]
fn _doc_create_post() {}

#[utoipa::path(
    post,
    path = "/threads/{id}/download",
    tag = "threads",
    description = "Download a thread snapshot blob from a peer using its stored ticket.",
    params(("id" = String, Path)),
    responses((status = 200, body = ThreadDetails))
)]
fn _doc_download_thread() {}

#[utoipa::path(
    get,
    path = "/posts/recent",
    tag = "threads",
    params(("limit" = Option<usize>, Query)),
    responses((status = 200, description = "Recent posts across all threads"))
)]
fn _doc_recent_posts() {}

#[utoipa::path(
    get,
    path = "/peers",
    tag = "peers",
    responses((status = 200, body = Vec<PeerView>))
)]
fn _doc_list_peers() {}

#[utoipa::path(
    post,
    path = "/peers",
    tag = "peers",
    request_body = AddPeerRequest,
    responses((status = 201, body = PeerView))
)]
fn _doc_add_peer() {}

#[utoipa::path(
    get,
    path = "/peers/self",
    tag = "peers",
    responses((status = 200, body = PeerView))
)]
fn _doc_self_peer() {}

#[utoipa::path(
    get,
    path = "/search",
    tag = "search",
    params(
        ("q" = String, Query, description = "Full-text search query"),
        ("limit" = Option<usize>, Query)
    ),
    responses((status = 200, description = "Search hits (posts and files)"))
)]
fn _doc_search() {}

#[utoipa::path(
    get,
    path = "/files/{id}",
    tag = "files",
    params(("id" = String, Path)),
    responses((status = 200, description = "Binary file content"))
)]
fn _doc_get_file() {}

// --- OpenAPI document ------------------------------------------------------

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Graphchan / OrbWeaver REST API",
        description = "Decentralized P2P forum and fileshare. The local node serves this REST API; agents drive it via /events (live updates) and the documented endpoints. Authentication: when GRAPHCHAN_API_TOKEN is set, all routes except /health and /openapi.json require Authorization: Bearer <token>. The bundled desktop launcher mints a per-launch token automatically.",
        version = env!("CARGO_PKG_VERSION"),
    ),
    paths(
        _doc_health,
        _doc_events,
        _doc_list_threads,
        _doc_get_thread,
        _doc_create_post,
        _doc_download_thread,
        _doc_recent_posts,
        _doc_list_peers,
        _doc_add_peer,
        _doc_self_peer,
        _doc_search,
        _doc_get_file,
    ),
    components(schemas(
        ThreadSummary,
        PostView,
        FileView,
        ThreadDetails,
        PeerView,
        HealthResponse,
        IdentityInfo,
        NetworkInfo,
        CreatePostInput,
        PostResponse,
        AddPeerRequest,
        ErrorResponse,
    )),
    tags(
        (name = "system", description = "Health, identity, live events"),
        (name = "threads", description = "Threads and posts"),
        (name = "peers", description = "Peer discovery and identity"),
        (name = "files", description = "Attachments and blobs"),
        (name = "search", description = "Full-text search across posts and files"),
    )
)]
pub struct ApiDoc;

pub async fn openapi_handler() -> axum::Json<utoipa::openapi::OpenApi> {
    axum::Json(ApiDoc::openapi())
}
