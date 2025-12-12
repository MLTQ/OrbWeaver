use crate::blocking::{
    BlockChecker, BlockedPeerView, BlocklistEntryView, BlocklistSubscriptionView,
};
use crate::config::GraphchanConfig;
use crate::database::Database;
use crate::database::repositories::{ThreadRepository, PeerRepository};
use crate::dms::{ConversationView, DirectMessageView, DmService};
use crate::files::{FileService, FileView, SaveFileInput};
use crate::identity::decode_friendcode;
use crate::identity::IdentitySummary;
use crate::network::FileAnnouncement;
use crate::network::NetworkHandle;
use crate::network::ProfileUpdate;
use crate::peers::{PeerService, PeerView};
use crate::threading::{
    CreatePostInput, CreateThreadInput, ThreadDetails, ThreadService, ThreadSummary,
};
use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart, Path, Query, State};
use axum::http::{
    header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    HeaderValue, StatusCode,
};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use iroh_blobs::{BlobFormat, Hash};
use rusqlite::OptionalExtension;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::str::FromStr;
use tokio::fs::File as TokioFile;
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;
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

#[derive(Debug, Deserialize)]
pub struct UpdateProfileRequest {
    pub username: Option<String>,
    pub bio: Option<String>,
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

    // Increase body limit to 50MB for file uploads (4chan images can be large)
    let router = Router::new()
        .route("/health", get(health_handler))
        .route("/threads", get(list_threads).post(create_thread))
        .route("/threads/:id", get(get_thread))
        .route("/threads/:id/download", post(download_thread))
        .route("/threads/:id/delete", post(delete_thread))
        .route("/threads/:id/ignore", post(set_thread_ignored))
        .route("/threads/:id/posts", post(create_post))
        .route("/posts/:id/files", get(list_post_files))
        .route("/posts/:id/files", post(upload_post_file))
        .route("/posts/:id/reactions", get(get_post_reactions))
        .route("/posts/:id/react", post(add_reaction))
        .route("/posts/:id/unreact", post(remove_reaction))
        .route("/files/:id", get(download_file))
        .route("/peers", get(list_peers))
        .route("/peers", post(add_peer))
        .route("/peers/:id/unfollow", post(unfollow_peer))
        .route("/peers/self", get(get_self_peer))
        .route("/identity/avatar", post(upload_avatar))
        .route("/identity/profile", post(update_profile_handler))
        .route("/blobs/:blob_id", get(get_blob))
        .route("/import", post(import_thread_handler))
        .route("/dms/conversations", get(list_conversations_handler))
        .route("/dms/send", post(send_dm_handler))
        .route("/dms/:peer_id/messages", get(get_messages_handler))
        .route("/dms/messages/:message_id/read", post(mark_message_read_handler))
        .route("/dms/unread/count", get(count_unread_handler))
        .route("/blocking/peers", get(list_blocked_peers_handler))
        .route("/blocking/peers/:peer_id", post(block_peer_handler))
        .route("/blocking/peers/:peer_id", axum::routing::delete(unblock_peer_handler))
        .route("/blocking/blocklists", get(list_blocklists_handler))
        .route("/blocking/blocklists", post(subscribe_blocklist_handler))
        .route("/blocking/blocklists/:id", axum::routing::delete(unsubscribe_blocklist_handler))
        .route("/blocking/blocklists/:id/entries", get(list_blocklist_entries_handler))
        .layer(DefaultBodyLimit::max(50 * 1024 * 1024)) // 50MB limit
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state.clone());

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

async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        api_port: state.config.api_port,
        identity: IdentityInfo {
            gpg_fingerprint: state.identity.gpg_fingerprint.clone(),
            iroh_peer_id: state.identity.iroh_peer_id.clone(),
            friendcode: state.identity.friendcode.clone(),
        },
        network: NetworkInfo::from_handle(&state.network),
    })
}

async fn list_threads(
    State(state): State<AppState>,
    Query(params): Query<ListThreadsParams>,
) -> ApiResult<Vec<ThreadSummary>> {
    let service = ThreadService::new(state.database.clone());
    let limit = params.limit.unwrap_or(50).min(200);
    let threads = service.list_threads(limit)?;
    Ok(Json(threads))
}

async fn get_thread(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<ThreadDetails> {
    let service = ThreadService::with_file_paths(state.database.clone(), state.config.paths.clone());
    match service.get_thread(&id)? {
        Some(thread) => Ok(Json(thread)),
        None => Err(ApiError::NotFound(format!("thread {id} not found"))),
    }
}

async fn download_thread(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
) -> Result<Json<ThreadDetails>, ApiError> {
    tracing::info!(thread_id = %thread_id, "📥 downloading thread from peer");

    // Get the blob ticket for this thread
    let ticket_str: Option<String> = state.database.with_repositories(|repos| {
        repos.conn().query_row(
            "SELECT ticket FROM thread_tickets WHERE thread_id = ?1",
            rusqlite::params![thread_id],
            |row| row.get::<_, String>(0),
        )
        .optional()
        .context("failed to query thread_tickets")
    })
    .map_err(ApiError::Internal)?;

    let Some(ticket_str) = ticket_str else {
        return Err(ApiError::NotFound(format!("no download ticket found for thread {thread_id}")));
    };

    // Parse the blob ticket
    let ticket = BlobTicket::from_str(&ticket_str)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("invalid blob ticket: {}", e)))?;

    // Download the blob
    tracing::info!(thread_id = %thread_id, "downloading blob from peer");
    let hash = ticket.hash();
    let endpoint = state.network.endpoint();

    // Check if we already have the blob
    let has_blob = state.blobs.has(hash).await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("failed to check blob existence: {}", e)))?;

    if !has_blob {
        // Download from peer
        let downloader = state.blobs.downloader(&endpoint);
        downloader.download(hash, Some(ticket.addr().id))
            .await
            .map_err(|e| ApiError::Internal(anyhow::anyhow!("blob download failed: {}", e)))?;
    }

    // Read the blob data
    let mut reader = state.blobs.reader(hash);
    let mut blob_data = Vec::new();
    tokio::io::copy(&mut reader, &mut blob_data)
        .await
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("failed to read blob: {}", e)))?;

    // Deserialize as ThreadDetails
    let thread_details: ThreadDetails = serde_json::from_slice(&blob_data)
        .map_err(|e| ApiError::Internal(anyhow::anyhow!("invalid thread data: {}", e)))?;

    tracing::info!(
        thread_id = %thread_id,
        posts = thread_details.posts.len(),
        "✓ downloaded thread, applying to database"
    );

    // Apply the thread to the database using apply_thread_snapshot
    crate::network::ingest::apply_thread_from_download(
        &state.database,
        &state.config.paths,
        &state.network,
        thread_details.clone(),
        &state.blobs,
    )
    .await
    .map_err(ApiError::Internal)?;

    // Delete the ticket after successful download
    state.database.with_repositories(|repos| {
        repos.conn().execute(
            "DELETE FROM thread_tickets WHERE thread_id = ?1",
            rusqlite::params![thread_id],
        )
        .context("failed to delete thread ticket")
    })
    .map_err(ApiError::Internal)?;

    // Subscribe to the thread topic to receive future updates
    if let Err(err) = state.network.subscribe_to_thread(&thread_id).await {
        tracing::warn!(error = ?err, thread_id = %thread_id, "failed to subscribe to thread topic after download");
    }

    tracing::info!(thread_id = %thread_id, "✓ thread download complete");

    Ok(Json(thread_details))
}

async fn create_thread(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<ThreadDetails>, ApiError> {
    let mut input: Option<CreateThreadInput> = None;
    let mut files: Vec<(String, String, Vec<u8>)> = Vec::new();

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "json" {
            let data = field
                .bytes()
                .await
                .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?;
            let parsed: CreateThreadInput =
                serde_json::from_slice(&data).map_err(|e| ApiError::BadRequest(e.to_string()))?;
            input = Some(parsed);
        } else if name == "file" {
            let filename = field.file_name().unwrap_or("unknown").to_string();
            let mime = field
                .content_type()
                .unwrap_or("application/octet-stream")
                .to_string();
            let data = field
                .bytes()
                .await
                .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?;
            files.push((filename, mime, data.to_vec()));
        }
    }

    let mut input = input.ok_or(ApiError::BadRequest("missing json field".into()))?;

    // If no creator specified, use the local peer (GPG fingerprint is the peer ID)
    if input.creator_peer_id.is_none() {
        input.creator_peer_id = Some(state.identity.gpg_fingerprint.clone());
    }

    let thread_service = ThreadService::new(state.database.clone());
    let details = thread_service
        .create_thread(input)
        .map_err(ApiError::Internal)?;

    // Attach files to the first post
    if !files.is_empty() {
        let file_service = FileService::new(
            state.database.clone(),
            state.config.paths.clone(),
            state.config.file.clone(),
            state.blobs.clone(),
        );
        let post_id = &details.posts[0].id;
        let addr = state.network.current_addr();

        for (filename, mime, data) in files {
            match file_service
                .save_post_file(SaveFileInput {
                    post_id: post_id.clone(),
                    original_name: Some(filename),
                    mime: Some(mime),
                    data,
                })
                .await
            {
                Ok(mut file_view) => {
                    let ticket = file_view.blob_id.as_deref().and_then(|blob_id| {
                        Hash::from_str(blob_id).ok().map(|hash| {
                            BlobTicket::new(addr.clone(), hash, BlobFormat::Raw)
                        })
                    });
                    if let Some(t) = &ticket {
                        file_service.persist_ticket(&file_view.id, Some(t)).ok();
                        file_view.ticket = Some(t.to_string());
                    }
                    
                    // Broadcast file available
                    if let (Some(blob_id), Some(ticket)) = (&file_view.blob_id, &ticket) {
                         if let Ok(_hash) = Hash::from_str(blob_id) {
                             let announcement = FileAnnouncement {
                                 id: file_view.id.clone(),
                                 post_id: file_view.post_id.clone(),
                                 thread_id: details.thread.id.clone(),
                                 original_name: file_view.original_name.clone(),
                                 mime: file_view.mime.clone(),
                                 size_bytes: file_view.size_bytes,
                                 checksum: file_view.checksum.clone(),
                                 blob_id: file_view.blob_id.clone(),
                                 ticket: Some(ticket.clone()),
                             };
                             tracing::info!(
                                 file_id = %file_view.id,
                                 post_id = %file_view.post_id,
                                 "📢 broadcasting FileAnnouncement (thread creation)"
                             );
                             state.network.publish_file_available(announcement).await.ok();
                         }
                    }
                }
                Err(e) => {
                    tracing::error!("Failed to save file for thread: {}", e);
                }
            }
        }
    }
    
    // Broadcast thread announcement
    state.network.publish_thread_announcement(details.clone(), &state.identity.gpg_fingerprint).await.ok();

    Ok(Json(details))
}

async fn create_post(
    State(state): State<AppState>,
    Path(thread_id): Path<String>,
    Json(mut payload): Json<CreatePostInput>,
) -> Result<(StatusCode, Json<PostResponse>), ApiError> {
    let service = ThreadService::new(state.database.clone());
    payload.thread_id = thread_id.clone();

    // If no author specified, use the local peer (GPG fingerprint is the peer ID)
    if payload.author_peer_id.is_none() {
        payload.author_peer_id = Some(state.identity.gpg_fingerprint.clone());
    }

    match service.create_post(payload) {
        Ok(post) => {
            // Broadcast the individual post update
            let outbound = post.clone();
            if let Err(err) = state.network.publish_post_update(outbound).await {
                tracing::warn!(
                    error = ?err,
                    thread_id = %post.thread_id,
                    post_id = %post.id,
                    "failed to publish post update over network"
                );
            }

            // Note: We only broadcast the PostUpdate, not the full thread.
            // If a peer doesn't have the thread yet, they'll see the post references it
            // and can request the thread on-demand.

            Ok((StatusCode::CREATED, Json(PostResponse { post })))
        }
        Err(err) if err.to_string().contains("thread not found") => {
            Err(ApiError::NotFound(format!("thread {thread_id} not found")))
        }
        Err(err) if err.to_string().contains("may not be empty") => {
            Err(ApiError::BadRequest(err.to_string()))
        }
        Err(err) => Err(ApiError::Internal(err)),
    }
}

async fn list_post_files(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
    Query(params): Query<ListFilesParams>,
) -> ApiResult<Vec<FileResponse>> {
    let service = FileService::new(
        state.database.clone(),
        state.config.paths.clone(),
        state.config.file.clone(),
        state.blobs.clone(),
    );
    let mut files = service.list_post_files(&post_id)?;
    if params.missing_only.unwrap_or(false) {
        files.retain(|f| !f.present.unwrap_or(true));
    }
    if let Some(mime_filter) = params.mime {
        files.retain(|f| f.mime.as_deref() == Some(mime_filter.as_str()));
    }
    let responses = files.into_iter().map(map_file_view).collect();
    Ok(Json(responses))
}

async fn upload_post_file(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<FileResponse>), ApiError> {
    let service = FileService::new(
        state.database.clone(),
        state.config.paths.clone(),
        state.config.file.clone(),
        state.blobs.clone(),
    );
    let mut file_bytes = None;
    let mut filename = None;
    let mut mime = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?
    {
        if let Some(name) = field.name() {
            if name == "file" {
                filename = field.file_name().map(|s| s.to_string());
                mime = field.content_type().map(|s| s.to_string());
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?;
                file_bytes = Some(bytes.to_vec());
                break;
            }
        }
    }

    let data = file_bytes.ok_or_else(|| ApiError::BadRequest("missing file field".into()))?;
    match service
        .save_post_file(SaveFileInput {
            post_id: post_id.clone(),
            original_name: filename,
            mime,
            data,
        })
        .await
    {
        Ok(mut file_view) => {
            let ticket = file_view
                .blob_id
                .as_deref()
                .and_then(|blob| state.network.make_blob_ticket(blob));
            file_view.ticket = ticket.as_ref().map(|t| t.to_string());
            let thread_service = ThreadService::new(state.database.clone());
            let thread_id = thread_service
                .get_post(&post_id)
                .map_err(ApiError::Internal)?
                .map(|p| p.thread_id)
                .unwrap_or_default(); // Should ideally handle not found, but we are in success path of save_post_file which checks post existence

            let announcement = FileAnnouncement {
                id: file_view.id.clone(),
                post_id: file_view.post_id.clone(),
                thread_id,
                original_name: file_view.original_name.clone(),
                mime: file_view.mime.clone(),
                size_bytes: file_view.size_bytes,
                checksum: file_view.checksum.clone(),
                blob_id: file_view.blob_id.clone(),
                ticket: ticket.clone(),
            };
            if let Err(err) = service.persist_ticket(&file_view.id, ticket.as_ref()) {
                tracing::warn!(error = ?err, file_id = %file_view.id, "failed to persist blob ticket");
            }
            tracing::info!(
                file_id = %file_view.id,
                post_id = %post_id,
                "📢 broadcasting FileAnnouncement"
            );
            if let Err(err) = state.network.publish_file_available(announcement).await {
                tracing::warn!(
                    error = ?err,
                    post_id = %post_id,
                    file_id = %file_view.id,
                    "failed to publish file availability over network"
                );
            }
            Ok((StatusCode::CREATED, Json(map_file_view(file_view))))
        }
        Err(err) if err.to_string().contains("post not found") => {
            Err(ApiError::NotFound(format!("post {post_id} not found")))
        }
        Err(err) => Err(ApiError::Internal(err)),
    }
}

async fn download_file(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let service = FileService::new(
        state.database.clone(),
        state.config.paths.clone(),
        state.config.file.clone(),
        state.blobs.clone(),
    );
    let Some(download) = service
        .prepare_download(&id)
        .await
        .map_err(ApiError::Internal)?
    else {
        return Err(ApiError::NotFound(format!("file {id} not found")));
    };

    let file = TokioFile::open(&download.absolute_path)
        .await
        .with_context(|| format!("unable to open {}", download.absolute_path.display()))
        .map_err(ApiError::Internal)?;
    let stream = ReaderStream::new(file);
    let body = Body::from_stream(stream);
    let mut response = Response::new(body);
    let headers = response.headers_mut();

    let mut content_type = download
        .metadata
        .mime
        .clone()
        .unwrap_or_else(|| "application/octet-stream".into());

    // If generic or missing, try to guess from extension
    if content_type == "application/octet-stream" {
        if let Some(name) = &download.metadata.original_name {
            if let Some(ext) = std::path::Path::new(name).extension().and_then(|e| e.to_str()) {
                let mime = match ext.to_lowercase().as_str() {
                    "jpg" | "jpeg" => "image/jpeg",
                    "png" => "image/png",
                    "gif" => "image/gif",
                    "webm" => "video/webm",
                    _ => "application/octet-stream",
                };
                content_type = mime.to_string();
            }
        }
    }

    if let Ok(value) = HeaderValue::from_str(&content_type) {
        headers.insert(CONTENT_TYPE, value);
    }

    if let Some(size) = download.metadata.size_bytes {
        if let Ok(value) = HeaderValue::from_str(&size.to_string()) {
            headers.insert(CONTENT_LENGTH, value);
        }
    }

    if let Some(name) = download.metadata.original_name.clone() {
        let safe = name.replace('"', "\"");
        let value = format!("attachment; filename=\"{}\"", safe);
        if let Ok(value) = HeaderValue::from_str(&value) {
            headers.insert(CONTENT_DISPOSITION, value);
        }
    }

    Ok(response)
}

async fn list_peers(State(state): State<AppState>) -> ApiResult<Vec<PeerView>> {
    let service = PeerService::new(state.database.clone());
    let peers = service.list_peers()?;
    Ok(Json(peers))
}

async fn get_self_peer(State(state): State<AppState>) -> ApiResult<Option<PeerView>> {
    let service = PeerService::new(state.database.clone());
    let peer = service.get_local_peer()?;
    Ok(Json(peer))
}

async fn add_peer(
    State(state): State<AppState>,
    Json(request): Json<AddPeerRequest>,
) -> Result<(StatusCode, Json<PeerView>), ApiError> {
    let service = PeerService::new(state.database.clone());
    let friendcode = request.friendcode.trim();
    match service.register_friendcode(friendcode) {
        Ok(peer) => {
            // Connect to the peer and get their iroh peer ID
            let iroh_peer_id = if let Ok(payload) = decode_friendcode(friendcode) {
                match state.network.connect_friendcode(&payload).await {
                    Ok(peer_id) => Some(peer_id),
                    Err(err) => {
                        tracing::warn!(error = ?err, "failed to connect to peer after registering friendcode");
                        None
                    }
                }
            } else {
                None
            };

            // Subscribe to this peer's topic to receive their announcements
            // Use the iroh peer ID as bootstrap to help establish gossip connectivity
            if let Err(err) = state.network.subscribe_to_peer(&peer.id, iroh_peer_id).await {
                tracing::warn!(error = ?err, peer_id = %peer.id, "failed to subscribe to peer topic");
            }

            Ok((StatusCode::CREATED, Json(peer)))
        }
        Err(err) if err.to_string().contains("decode friendcode") => {
            Err(ApiError::BadRequest("invalid friendcode".into()))
        }
        Err(err) => Err(ApiError::Internal(err)),
    }
}

async fn import_thread_handler(
    State(state): State<AppState>,
    Json(request): Json<ImportRequest>,
) -> Result<(StatusCode, Json<ImportResponse>), ApiError> {
    let result = if request.platform.as_deref() == Some("reddit") {
        crate::importer::import_reddit_thread(&state, &request.url).await
    } else {
        crate::importer::import_fourchan_thread(&state, &request.url).await
    };

    match result {
        Ok(id) => Ok((StatusCode::CREATED, Json(ImportResponse { id }))),
        Err(e) => {
            tracing::error!("Import failed: {}", e);
            Err(ApiError::Internal(e))
        }
    }
}

#[derive(Deserialize)]
struct ImportRequest {
    url: String,
    platform: Option<String>,
}

#[derive(Serialize)]
struct ImportResponse {
    id: String,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
    api_port: u16,
    identity: IdentityInfo,
    network: NetworkInfo,
}

#[derive(Serialize)]
struct IdentityInfo {
    gpg_fingerprint: String,
    iroh_peer_id: String,
    friendcode: String,
}

#[derive(Serialize)]
struct NetworkInfo {
    peer_id: String,
    addresses: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ListThreadsParams {
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ListFilesParams {
    #[serde(default)]
    missing_only: Option<bool>,
    #[serde(default)]
    mime: Option<String>,
}

#[derive(Debug, Serialize)]
struct PostResponse {
    post: crate::threading::PostView,
}

#[derive(Debug, Serialize)]
struct FileResponse {
    id: String,
    post_id: String,
    original_name: Option<String>,
    mime: Option<String>,
    size_bytes: Option<i64>,
    checksum: Option<String>,
    blob_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    ticket: Option<String>,
    path: String,
    download_url: String,
    present: bool,
}

#[derive(Debug, Deserialize)]
struct AddPeerRequest {
    friendcode: String,
}

type ApiResult<T> = Result<Json<T>, ApiError>;

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

async fn upload_avatar(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<String>), ApiError> {
    let service = FileService::new(
        state.database.clone(),
        state.config.paths.clone(),
        state.config.file.clone(),
        state.blobs.clone(),
    );
    let mut file_bytes = None;

    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "file" {
            let data = field
                .bytes()
                .await
                .map_err(|err| ApiError::Internal(anyhow::Error::new(err)))?;
            file_bytes = Some(data.to_vec());
            tracing::info!("Avatar file received, size: {}", data.len());
            break;
        } else {
            tracing::debug!("Ignored field in avatar upload: {}", name);
        }
    }

    let Some(bytes) = file_bytes else {
        return Err(ApiError::BadRequest("missing file field".into()));
    };

    let blob_id = service.import_blob(bytes).await.map_err(ApiError::Internal)?;

    // Update local peer profile
    let peer_service = PeerService::new(state.database.clone());
    // We need the local peer ID (fingerprint).
    // We can get it from state.identity.gpg_fingerprint.
    let peer_id = state.identity.gpg_fingerprint.clone();
    peer_service.update_profile(&peer_id, Some(blob_id.clone()), None, None).map_err(ApiError::Internal)?;

    // Generate ticket
    let hash = Hash::from_str(&blob_id).map_err(|e| ApiError::Internal(anyhow::anyhow!(e)))?;
    let addr = state.network.current_addr();
    let ticket = BlobTicket::new(addr, hash, BlobFormat::Raw);

    // Broadcast ProfileUpdate
    let update = ProfileUpdate {
        peer_id: peer_id.clone(),
        avatar_file_id: Some(blob_id.clone()),
        ticket: Some(ticket),
        username: None,
        bio: None,
    };
    state.network.publish_profile_update(update).await.map_err(ApiError::Internal)?;

    Ok((StatusCode::OK, Json(blob_id)))
}

async fn update_profile_handler(
    State(state): State<AppState>,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<StatusCode, ApiError> {
    let peer_service = PeerService::new(state.database.clone());
    let peer_id = state.identity.gpg_fingerprint.clone();
    
    peer_service.update_profile(&peer_id, None, payload.username.clone(), payload.bio.clone())
        .map_err(ApiError::Internal)?;

    // Broadcast ProfileUpdate
    // We need to fetch the current avatar ticket if we want to include it, 
    // or we can make the fields optional in ProfileUpdate too.
    // Let's assume ProfileUpdate needs to be updated to support optional fields.
    // For now, let's just send what we have.
    
    // We need to get the current avatar ticket to send it along, or send None if we don't want to change it.
    // But broadcast_profile_update replaces the state usually.
    // Let's check ProfileUpdate struct in network.rs.
    
    let update = ProfileUpdate {
        peer_id: peer_id.clone(),
        avatar_file_id: None,
        ticket: None,
        username: payload.username,
        bio: payload.bio,
    };
    state.network.publish_profile_update(update).await.map_err(ApiError::Internal)?;

    Ok(StatusCode::OK)
}

async fn get_blob(
    State(state): State<AppState>,
    Path(blob_id): Path<String>,
) -> Result<Response, ApiError> {
    let hash = Hash::from_str(&blob_id).map_err(|_| ApiError::NotFound("invalid blob id".into()))?;
    let reader = state.blobs.reader(hash);
    let stream = ReaderStream::new(reader);
    let body = Body::from_stream(stream);
    
    let mut headers = axum::http::HeaderMap::new();
    // We don't know the mime type unless we store it or infer it.
    // For now, let's assume generic binary or try to infer from first bytes if possible (hard with stream).
    // Or just let the browser guess.
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/octet-stream"));

    Ok((headers, body).into_response())
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

fn map_file_view(file: FileView) -> FileResponse {
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
    }
}

impl NetworkInfo {
    fn from_handle(handle: &NetworkHandle) -> Self {
        let addr = handle.current_addr();
        let mut addresses = Vec::new();
        for ip in addr.ip_addrs() {
            addresses.push(ip.to_string());
        }
        for relay in addr.relay_urls() {
            addresses.push(relay.to_string());
        }
        Self {
            peer_id: handle.peer_id(),
            addresses,
        }
    }
}

// Thread management handlers

async fn delete_thread(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.database.with_repositories(|repos| {
        repos.threads().delete(&id)
    })?;
    Ok(StatusCode::OK)
}

#[derive(Debug, Deserialize)]
struct SetIgnoredRequest {
    ignored: bool,
}

async fn set_thread_ignored(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<SetIgnoredRequest>,
) -> Result<StatusCode, ApiError> {
    state.database.with_repositories(|repos| {
        repos.threads().set_ignored(&id, payload.ignored)
    })?;
    Ok(StatusCode::OK)
}

async fn unfollow_peer(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.database.with_repositories(|repos| {
        repos.peers().delete(&id)
    })?;
    Ok(StatusCode::OK)
}

// Reaction handlers

#[derive(Debug, Deserialize)]
struct AddReactionRequest {
    emoji: String,
}

#[derive(Debug, Deserialize)]
struct RemoveReactionRequest {
    emoji: String,
}

#[derive(Debug, Serialize)]
struct ReactionsResponse {
    reactions: Vec<ReactionView>,
    counts: std::collections::HashMap<String, usize>,
}

#[derive(Debug, Serialize)]
struct ReactionView {
    emoji: String,
    reactor_peer_id: String,
    created_at: String,
}

async fn get_post_reactions(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
) -> ApiResult<ReactionsResponse> {
    use crate::database::repositories::ReactionRepository;

    let (reactions, counts) = state.database.with_repositories(|repos| {
        let reactions = repos.reactions().list_for_post(&post_id)?;
        let counts = repos.reactions().count_for_post(&post_id)?;
        Ok((reactions, counts))
    })?;

    let reaction_views: Vec<ReactionView> = reactions
        .into_iter()
        .map(|r| ReactionView {
            emoji: r.emoji,
            reactor_peer_id: r.reactor_peer_id,
            created_at: r.created_at,
        })
        .collect();

    Ok(Json(ReactionsResponse {
        reactions: reaction_views,
        counts,
    }))
}

async fn add_reaction(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
    Json(payload): Json<AddReactionRequest>,
) -> Result<StatusCode, ApiError> {
    use crate::database::repositories::ReactionRepository;
    use crate::database::models::ReactionRecord;

    // Get local identity to sign the reaction
    let identity: String = state.database.with_repositories(|repos| {
        let result: Result<String, rusqlite::Error> = repos.conn().query_row(
            "SELECT gpg_fingerprint FROM node_identity WHERE id = 1",
            [],
            |row| row.get(0),
        );
        result.map_err(|e| anyhow::Error::from(e))
    })?;

    // Create signature (simplified - in production would use GPG)
    let signature = format!("sig:{}:{}:{}", post_id, identity, payload.emoji);
    let created_at = chrono::Utc::now().to_rfc3339();

    let reaction = ReactionRecord {
        post_id: post_id.clone(),
        reactor_peer_id: identity,
        emoji: payload.emoji,
        signature,
        created_at,
    };

    state.database.with_repositories(|repos| {
        repos.reactions().add(&reaction)
    })?;

    // Get thread_id for the post
    let thread_id: String = state.database.with_repositories(|repos| {
        use crate::database::repositories::PostRepository;
        let post = repos.posts().get(&post_id)?;
        Ok(post.ok_or_else(|| anyhow::anyhow!("Post not found"))?.thread_id)
    })?;

    // Broadcast via gossip
    let reaction_update = crate::network::ReactionUpdate {
        post_id,
        thread_id,
        reactor_peer_id: reaction.reactor_peer_id.clone(),
        emoji: reaction.emoji.clone(),
        signature: reaction.signature.clone(),
        created_at: reaction.created_at.clone(),
        is_removal: false,
    };

    state.network.publish_reaction_update(reaction_update).await?;

    Ok(StatusCode::OK)
}

async fn remove_reaction(
    State(state): State<AppState>,
    Path(post_id): Path<String>,
    Json(payload): Json<RemoveReactionRequest>,
) -> Result<StatusCode, ApiError> {
    use crate::database::repositories::ReactionRepository;

    let identity: String = state.database.with_repositories(|repos| {
        let result: Result<String, rusqlite::Error> = repos.conn().query_row(
            "SELECT gpg_fingerprint FROM node_identity WHERE id = 1",
            [],
            |row| row.get(0),
        );
        result.map_err(|e| anyhow::Error::from(e))
    })?;

    state.database.with_repositories(|repos| {
        repos.reactions().remove(&post_id, &identity, &payload.emoji)
    })?;

    // Get thread_id for the post
    let thread_id: String = state.database.with_repositories(|repos| {
        use crate::database::repositories::PostRepository;
        let post = repos.posts().get(&post_id)?;
        Ok(post.ok_or_else(|| anyhow::anyhow!("Post not found"))?.thread_id)
    })?;

    // Broadcast unreact via gossip
    let reaction_update = crate::network::ReactionUpdate {
        post_id,
        thread_id,
        reactor_peer_id: identity,
        emoji: payload.emoji,
        signature: "".to_string(), // Not needed for removal
        created_at: chrono::Utc::now().to_rfc3339(),
        is_removal: true,
    };

    state.network.publish_reaction_update(reaction_update).await?;

    Ok(StatusCode::OK)
}

// Direct Message handlers

#[derive(Debug, Deserialize)]
struct SendDmRequest {
    to_peer_id: String,
    body: String,
}

#[derive(Debug, Deserialize)]
struct GetMessagesParams {
    #[serde(default = "default_limit")]
    limit: usize,
}

fn default_limit() -> usize {
    50
}

#[derive(Debug, Serialize)]
struct UnreadCountResponse {
    count: usize,
}

async fn list_conversations_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<ConversationView>> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let conversations = service.list_conversations().map_err(ApiError::Internal)?;
    Ok(Json(conversations))
}

async fn send_dm_handler(
    State(state): State<AppState>,
    Json(payload): Json<SendDmRequest>,
) -> Result<(StatusCode, Json<DirectMessageView>), ApiError> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let message = service
        .send_dm(&payload.to_peer_id, &payload.body)
        .map_err(ApiError::Internal)?;

    // TODO: Broadcast DM over gossip to recipient
    // This will be implemented when DM gossip events are added

    Ok((StatusCode::CREATED, Json(message)))
}

async fn get_messages_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
    Query(params): Query<GetMessagesParams>,
) -> ApiResult<Vec<DirectMessageView>> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let limit = params.limit.min(200);
    let messages = service
        .get_messages(&peer_id, limit)
        .map_err(ApiError::Internal)?;
    Ok(Json(messages))
}

async fn mark_message_read_handler(
    State(state): State<AppState>,
    Path(message_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    service
        .mark_as_read(&message_id)
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::OK)
}

async fn count_unread_handler(
    State(state): State<AppState>,
) -> ApiResult<UnreadCountResponse> {
    let service = DmService::new(state.database.clone(), state.config.paths.clone());
    let count = service.count_unread().map_err(ApiError::Internal)?;
    Ok(Json(UnreadCountResponse { count }))
}

// Blocking and Moderation handlers

#[derive(Debug, Deserialize)]
struct BlockPeerRequest {
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SubscribeBlocklistRequest {
    maintainer_peer_id: String,
    name: String,
    description: Option<String>,
    #[serde(default = "default_auto_apply")]
    auto_apply: bool,
}

fn default_auto_apply() -> bool {
    true
}

async fn list_blocked_peers_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<BlockedPeerView>> {
    let checker = BlockChecker::new(state.database.clone());
    let blocked = checker.list_blocked_peers().map_err(ApiError::Internal)?;
    Ok(Json(blocked))
}

async fn block_peer_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
    Json(payload): Json<BlockPeerRequest>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());
    checker
        .block_peer(&peer_id, payload.reason)
        .map_err(ApiError::Internal)?;

    // TODO: Broadcast block action to network (optional - for shared blocklists)

    Ok(StatusCode::OK)
}

async fn unblock_peer_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());
    checker.unblock_peer(&peer_id).map_err(ApiError::Internal)?;
    Ok(StatusCode::OK)
}

async fn list_blocklists_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<BlocklistSubscriptionView>> {
    let checker = BlockChecker::new(state.database.clone());
    let blocklists = checker
        .list_blocklist_subscriptions()
        .map_err(ApiError::Internal)?;
    Ok(Json(blocklists))
}

async fn subscribe_blocklist_handler(
    State(state): State<AppState>,
    Json(payload): Json<SubscribeBlocklistRequest>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());

    // Generate a blocklist ID from maintainer + name
    let blocklist_id = format!(
        "{}",
        blake3::hash(format!("blocklist:{}:{}", payload.maintainer_peer_id, payload.name).as_bytes())
    );

    checker
        .subscribe_blocklist(
            &blocklist_id,
            &payload.maintainer_peer_id,
            payload.name,
            payload.description,
            payload.auto_apply,
        )
        .map_err(ApiError::Internal)?;

    // TODO: Subscribe to blocklist updates from maintainer via gossip

    Ok(StatusCode::CREATED)
}

async fn unsubscribe_blocklist_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());
    checker
        .unsubscribe_blocklist(&id)
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::OK)
}

async fn list_blocklist_entries_handler(
    State(state): State<AppState>,
    Path(blocklist_id): Path<String>,
) -> ApiResult<Vec<BlocklistEntryView>> {
    let checker = BlockChecker::new(state.database.clone());
    let entries = checker
        .list_blocklist_entries(&blocklist_id)
        .map_err(ApiError::Internal)?;
    Ok(Json(entries))
}
