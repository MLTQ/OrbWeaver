mod blocking;
mod dms;
mod events;
mod files;
mod openapi;
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
use axum::extract::{DefaultBodyLimit, Request, State};
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::{self, Next};
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
    let max_upload_bytes = config
        .file
        .max_upload_bytes
        .unwrap_or(10 * 1024 * 1024 * 1024);

    // Auth setup: derive whether we run authenticated based on config + bind address.
    let auth_token = config.auth.token.clone();
    if auth_token.is_some() {
        tracing::info!("REST API auth enabled (Bearer token required)");
    } else {
        tracing::warn!(
            "REST API running without auth — set GRAPHCHAN_API_TOKEN to require Bearer token"
        );
    }

    let router = Router::new()
        .route("/health", get(threads::health_handler))
        .route("/openapi.json", get(openapi::openapi_handler))
        .route("/events", get(events::stream_events))
        .route(
            "/threads",
            get(threads::list_threads).post(threads::create_thread),
        )
        .route("/threads/:id", get(threads::get_thread))
        .route("/threads/:id/download", post(threads::download_thread))
        .route(
            "/threads/:id/refresh",
            post(threads::refresh_thread_handler),
        )
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
        .route(
            "/identity/agents/:name",
            delete(peers::remove_agent_handler),
        )
        .route("/identity/theme_color", get(peers::get_theme_color_handler))
        .route(
            "/identity/theme_color",
            post(peers::set_theme_color_handler),
        )
        .route("/blobs/:blob_id", get(files::get_blob))
        .route("/import", post(threads::import_thread_handler))
        .route("/dms/conversations", get(dms::list_conversations_handler))
        .route("/dms/send", post(dms::send_dm_handler))
        .route("/dms/:peer_id/messages", get(dms::get_messages_handler))
        .route(
            "/dms/messages/:message_id/read",
            post(dms::mark_message_read_handler),
        )
        .route(
            "/dms/:peer_id/read",
            post(dms::mark_conversation_read_handler),
        )
        .route("/dms/unread/count", get(dms::count_unread_handler))
        .route("/blocking/peers", get(blocking::list_blocked_peers_handler))
        .route(
            "/blocking/peers/:peer_id",
            post(blocking::block_peer_handler),
        )
        .route(
            "/blocking/peers/:peer_id",
            delete(blocking::unblock_peer_handler),
        )
        .route(
            "/blocking/peers/export",
            get(blocking::export_peer_blocks_handler),
        )
        .route(
            "/blocking/peers/import",
            post(blocking::import_peer_blocks_handler),
        )
        .route(
            "/blocking/blocklists",
            get(blocking::list_blocklists_handler),
        )
        .route(
            "/blocking/blocklists",
            post(blocking::subscribe_blocklist_handler),
        )
        .route(
            "/blocking/blocklists/:id",
            delete(blocking::unsubscribe_blocklist_handler),
        )
        .route(
            "/blocking/blocklists/:id/entries",
            get(blocking::list_blocklist_entries_handler),
        )
        .route("/blocking/ips", get(blocking::list_ip_blocks_handler))
        .route("/blocking/ips", post(blocking::add_ip_block_handler))
        .route(
            "/blocking/ips/:id",
            delete(blocking::remove_ip_block_handler),
        )
        .route(
            "/blocking/ips/import",
            post(blocking::import_ip_blocks_handler),
        )
        .route(
            "/blocking/ips/export",
            get(blocking::export_ip_blocks_handler),
        )
        .route(
            "/blocking/ips/clear",
            post(blocking::clear_all_ip_blocks_handler),
        )
        .route("/blocking/ips/stats", get(blocking::ip_block_stats_handler))
        .route("/peers/:peer_id/ip", get(blocking::get_peer_ip_handler))
        .route("/search", get(search::search_handler))
        .route(
            "/settings/:key",
            get(settings::get_setting_handler).put(settings::set_setting_handler),
        )
        .route(
            "/topics",
            get(settings::list_topics_handler).post(settings::subscribe_topic_handler),
        )
        .route(
            "/topics/:topic_id",
            delete(settings::unsubscribe_topic_handler),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            require_bearer_token,
        ))
        .layer(DefaultBodyLimit::max(max_upload_bytes as usize))
        .layer(build_cors_layer(&config))
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
    let network_for_shutdown = state.network.clone();
    axum::serve(listener, router.into_make_service())
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    tracing::info!("HTTP server stopped, shutting down network handle");
    network_for_shutdown.shutdown().await;
    Ok(())
}

/// Build the CORS layer from config. If `cors_origins` is set, restrict to those
/// origins; otherwise fall back to the legacy permissive `Any` origin (preserves
/// behavior for existing local installs where the desktop app is the only client).
fn build_cors_layer(config: &GraphchanConfig) -> CorsLayer {
    match &config.auth.cors_origins {
        Some(origins) => {
            let parsed: Vec<HeaderValue> = origins
                .iter()
                .filter_map(|o| o.parse::<HeaderValue>().ok())
                .collect();
            tracing::info!(allowed_origins = ?origins, "CORS restricted via GRAPHCHAN_CORS_ORIGINS");
            CorsLayer::new()
                .allow_origin(parsed)
                .allow_methods(Any)
                .allow_headers(Any)
        }
        None => CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any),
    }
}

/// Auth middleware. When `config.auth.token` is set, every request except those
/// on the always-public allowlist (currently `/health`) must carry a matching
/// `Authorization: Bearer <token>` header. Without a configured token, the
/// middleware is a no-op and all requests pass through.
async fn require_bearer_token(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let Some(expected) = state.config.auth.token.as_deref() else {
        return next.run(request).await;
    };

    let path = request.uri().path();
    if is_public_path(path) {
        return next.run(request).await;
    }

    let provided = request
        .headers()
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "));

    match provided {
        Some(token) if constant_time_eq(token.as_bytes(), expected.as_bytes()) => {
            next.run(request).await
        }
        _ => {
            tracing::debug!(path = %path, "rejecting unauthenticated request");
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    message: "missing or invalid bearer token".into(),
                }),
            )
                .into_response()
        }
    }
}

/// Routes that bypass auth (liveness/health checks, OpenAPI discovery).
fn is_public_path(path: &str) -> bool {
    matches!(path, "/health" | "/openapi.json")
}

/// Constant-time byte comparison to avoid leaking token length / prefix via
/// timing. Both sides must have identical lengths to compare equal.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}

/// Resolves on Ctrl-C (Unix and Windows). Used to drive `axum::serve`'s
/// `with_graceful_shutdown` so in-flight requests complete and the network
/// stack flushes before the process exits.
async fn shutdown_signal() {
    let ctrl_c = async {
        if let Err(err) = tokio::signal::ctrl_c().await {
            tracing::warn!(error = ?err, "failed to install Ctrl-C handler");
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(err) => {
                tracing::warn!(error = ?err, "failed to install SIGTERM handler");
                std::future::pending::<()>().await;
            }
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => tracing::info!("received Ctrl-C, shutting down"),
        _ = terminate => tracing::info!("received SIGTERM, shutting down"),
    }
}
