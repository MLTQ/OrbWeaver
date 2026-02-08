mod blocking;
mod dms;
mod files;
mod peers;
mod reactions;
mod search;
mod settings;
mod threads;

use crate::config::GraphchanConfig;
use crate::database::Database;
use crate::files::FileView;
use crate::identity::IdentitySummary;
use crate::network::NetworkHandle;
use anyhow::{Context, Result};
use axum::extract::DefaultBodyLimit;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::{Json, Router};
use iroh_blobs::store::fs::FsStore;
use serde::Serialize;
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tower_http::cors::{Any, CorsLayer};

#[derive(Clone)]
pub struct AppState {
    pub config: GraphchanConfig,
    pub identity: IdentitySummary,
    pub database: Database,
    pub network: NetworkHandle,
    pub blobs: FsStore,
    pub http_client: reqwest::Client,
}

pub(crate) type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    NotFound(String),
    Internal(anyhow::Error),
}

impl ApiError {
    fn into_response_parts(self) -> (StatusCode, ErrorResponse) {
        match self {
            ApiError::BadRequest(msg) => (StatusCode::BAD_REQUEST, ErrorResponse { message: msg }),
            ApiError::NotFound(msg) => (StatusCode::NOT_FOUND, ErrorResponse { message: msg }),
            ApiError::Internal(err) => {
                tracing::error!(error = ?err, "internal server error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    ErrorResponse {
                        message: "internal server error".into(),
                    },
                )
            }
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, body) = self.into_response_parts();
        (status, Json(body)).into_response()
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::Internal(err)
    }
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    message: String,
}

#[derive(Debug, Serialize)]
pub(crate) struct FileResponse {
    pub id: String,
    pub post_id: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub blob_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ticket: Option<String>,
    pub path: String,
    pub download_url: String,
    pub present: bool,
    pub download_status: Option<String>,
}

pub(crate) fn map_file_view(file: FileView) -> FileResponse {
    FileResponse {
        id: file.id.clone(),
        post_id: file.post_id.clone(),
        original_name: file.original_name.clone(),
        mime: file.mime.clone(),
        size_bytes: file.size_bytes,
        checksum: file.checksum.clone(),
        blob_id: file.blob_id.clone(),
        ticket: file.ticket.clone(),
        path: file.path.clone(),
        download_url: format!("/files/{}", file.id),
        present: file.present.unwrap_or(true),
        download_status: file.download_status.clone(),
    }
}

/// Tries to bind to the given port, or finds the next available port
async fn find_available_port(start_port: u16) -> Result<(TcpListener, u16)> {
    const MAX_PORT_ATTEMPTS: u16 = 100;

    for offset in 0..MAX_PORT_ATTEMPTS {
        let port = start_port + offset;
        let addr = SocketAddr::from(([0, 0, 0, 0], port));

        match TcpListener::bind(addr).await {
            Ok(listener) => return Ok((listener, port)),
            Err(e) => {
                if offset == 0 {
                    tracing::debug!(port, error = %e, "Port in use, trying next port");
                }
                continue;
            }
        }
    }

    anyhow::bail!(
        "Could not find available port in range {}-{}",
        start_port,
        start_port + MAX_PORT_ATTEMPTS - 1
    )
}

pub async fn serve_http(
    config: GraphchanConfig,
    identity: IdentitySummary,
    database: Database,
    network: NetworkHandle,
    blobs: FsStore,
) -> Result<()> {
    let http_client = reqwest::Client::builder()
        .user_agent("Graphchan/0.1.0")
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .context("failed to build shared HTTP client")?;

    let state = AppState {
        config: config.clone(),
        identity,
        database,
        network,
        blobs,
        http_client,
    };

    // Configure body limit for file uploads (default 10GB if not specified)
    // Media files (images/video/audio) are limited to 50MB at the handler level
    let max_upload_bytes = config.file.max_upload_bytes.unwrap_or(10 * 1024 * 1024 * 1024);
    let router = Router::new()
        .route("/health", get(threads::health_handler))
        .route("/threads", get(threads::list_threads).post(threads::create_thread))
        .route("/threads/:id", get(threads::get_thread))
        .route("/threads/:id/download", post(threads::download_thread))
        .route("/threads/:id/delete", post(threads::delete_thread))
        .route("/threads/:id/ignore", post(threads::set_thread_ignored))
        .route("/threads/:id/posts", post(threads::create_post))
        .route("/posts/recent", get(threads::list_recent_posts))
        .route("/posts/:id/files", get(files::list_post_files))
        .route("/posts/:id/files", post(files::upload_post_file))
        .route("/posts/:id/reactions", get(reactions::get_post_reactions))
        .route("/posts/:id/react", post(reactions::add_reaction))
        .route("/posts/:id/unreact", post(reactions::remove_reaction))
        .route("/files/:id", get(files::download_file))
        .route("/files/:id/download", post(files::trigger_file_download))
        .route("/peers", get(peers::list_peers))
        .route("/peers", post(peers::add_peer))
        .route("/peers/:id/unfollow", post(peers::unfollow_peer))
        .route("/peers/self", get(peers::get_self_peer))
        .route("/identity/avatar", post(peers::upload_avatar))
        .route("/identity/profile", post(peers::update_profile_handler))
        .route("/identity/agents", get(peers::get_agents_handler))
        .route("/identity/agents", post(peers::add_agent_handler))
        .route("/identity/agents/:name", delete(peers::remove_agent_handler))
        .route("/identity/theme_color", get(peers::get_theme_color_handler))
        .route("/identity/theme_color", post(peers::set_theme_color_handler))
        .route("/blobs/:blob_id", get(files::get_blob))
        .route("/import", post(threads::import_thread_handler))
        .route("/dms/conversations", get(dms::list_conversations_handler))
        .route("/dms/send", post(dms::send_dm_handler))
        .route("/dms/:peer_id/messages", get(dms::get_messages_handler))
        .route("/dms/messages/:message_id/read", post(dms::mark_message_read_handler))
        .route("/dms/unread/count", get(dms::count_unread_handler))
        .route("/blocking/peers", get(blocking::list_blocked_peers_handler))
        .route("/blocking/peers/:peer_id", post(blocking::block_peer_handler))
        .route("/blocking/peers/:peer_id", delete(blocking::unblock_peer_handler))
        .route("/blocking/peers/export", get(blocking::export_peer_blocks_handler))
        .route("/blocking/peers/import", post(blocking::import_peer_blocks_handler))
        .route("/blocking/blocklists", get(blocking::list_blocklists_handler))
        .route("/blocking/blocklists", post(blocking::subscribe_blocklist_handler))
        .route("/blocking/blocklists/:id", delete(blocking::unsubscribe_blocklist_handler))
        .route("/blocking/blocklists/:id/entries", get(blocking::list_blocklist_entries_handler))
        .route("/blocking/ips", get(blocking::list_ip_blocks_handler))
        .route("/blocking/ips", post(blocking::add_ip_block_handler))
        .route("/blocking/ips/:id", delete(blocking::remove_ip_block_handler))
        .route("/blocking/ips/import", post(blocking::import_ip_blocks_handler))
        .route("/blocking/ips/export", get(blocking::export_ip_blocks_handler))
        .route("/blocking/ips/clear", post(blocking::clear_all_ip_blocks_handler))
        .route("/blocking/ips/stats", get(blocking::ip_block_stats_handler))
        .route("/peers/:peer_id/ip", get(blocking::get_peer_ip_handler))
        .route("/search", get(search::search_handler))
        .route("/settings/:key", get(settings::get_setting_handler).put(settings::set_setting_handler))
        .route("/topics", get(settings::list_topics_handler).post(settings::subscribe_topic_handler))
        .route("/topics/:topic_id", delete(settings::unsubscribe_topic_handler))
        .layer(DefaultBodyLimit::max(max_upload_bytes as usize))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state.clone());

    tracing::info!(
        max_body_limit_mb = max_upload_bytes / (1024 * 1024),
        "Configured upload body limit (files >50MB won't auto-download on recipient)"
    );

    // Try to bind to the configured port, or find the next available port
    let (listener, actual_port) = find_available_port(config.api_port).await?;
    let addr = SocketAddr::from(([0, 0, 0, 0], actual_port));

    if actual_port != config.api_port {
        tracing::warn!(
            requested_port = config.api_port,
            actual_port = actual_port,
            "Configured port was in use, bound to next available port"
        );
    }

    tracing::info!(?addr, "HTTP server listening");
    axum::serve(listener, router.into_make_service()).await?;
    Ok(())
}
