use super::{AppState, ApiError, FileResponse, map_file_view};
use crate::database::Database;
use crate::database::repositories::{ThreadRepository, PostRepository, FileRepository};
use crate::files::{FileService, FileView};
use crate::identity::IdentitySummary;
use crate::network::{FileAnnouncement, NetworkHandle, DhtStatus};
use crate::threading::{CreatePostInput, CreateThreadInput, ThreadDetails, ThreadService, ThreadSummary};
use anyhow::{Context, Result};
use axum::extract::{Multipart, Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::{BlobFormat, Hash};
use serde::{Deserialize, Serialize};

use crate::files::SaveFileInput;
use iroh_blobs::ticket::BlobTicket;
use rusqlite::OptionalExtension;
use std::str::FromStr;

type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug, Deserialize)]
pub(crate) struct ListThreadsParams {
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
pub(crate) struct PostResponse {
    post: crate::threading::PostView,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecentPostView {
    pub post: crate::threading::PostView,
    pub thread_title: String,
    pub files: Vec<FileResponse>,
}

#[derive(Debug, Serialize)]
pub(crate) struct RecentPostsResponse {
    pub posts: Vec<RecentPostView>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RecentPostsParams {
    #[serde(default = "default_recent_limit")]
    limit: Option<usize>,
}

pub(crate) fn default_recent_limit() -> Option<usize> {
    Some(50)
}

#[derive(Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    version: &'static str,
    api_port: u16,
    identity: IdentityInfo,
    network: NetworkInfo,
}

#[derive(Serialize)]
pub(crate) struct IdentityInfo {
    gpg_fingerprint: String,
    iroh_peer_id: String,
    friendcode: String,
    short_friendcode: String,
}

#[derive(Serialize)]
pub(crate) struct NetworkInfo {
    peer_id: String,
    addresses: Vec<String>,
    dht_status: String,  // "checking", "connected", or "unreachable"
}

impl NetworkInfo {
    pub(crate) fn from_handle(handle: &NetworkHandle) -> Self {
        let addr = handle.current_addr();
        let mut addresses = Vec::new();
        for ip in addr.ip_addrs() {
            addresses.push(ip.to_string());
        }
        for relay in addr.relay_urls() {
            addresses.push(relay.to_string());
        }

        let dht_status = match handle.dht_status() {
            DhtStatus::Checking => "checking",
            DhtStatus::Connected => "connected",
            DhtStatus::Unreachable => "unreachable",
        }.to_string();

        Self {
            peer_id: handle.peer_id(),
            addresses,
            dht_status,
        }
    }
}

#[derive(Deserialize)]
pub(crate) struct ImportRequest {
    url: String,
    platform: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct ImportResponse {
    id: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SetIgnoredRequest {
    ignored: bool,
}

pub(crate) async fn health_handler(State(state): State<AppState>) -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: env!("CARGO_PKG_VERSION"),
        api_port: state.config.api_port,
        identity: IdentityInfo {
            gpg_fingerprint: state.identity.gpg_fingerprint.clone(),
            iroh_peer_id: state.identity.iroh_peer_id.clone(),
            friendcode: state.identity.friendcode.clone(),
            short_friendcode: state.identity.short_friendcode.clone(),
        },
        network: NetworkInfo::from_handle(&state.network),
    })
}

pub(crate) async fn list_threads(
    State(state): State<AppState>,
    Query(params): Query<ListThreadsParams>,
) -> ApiResult<Vec<ThreadSummary>> {
    let service = ThreadService::new(state.database.clone());
    let limit = params.limit.unwrap_or(50).min(200);
    let threads = service.list_threads(limit)?;
    Ok(Json(threads))
}

pub(crate) async fn get_thread(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> ApiResult<ThreadDetails> {
    let service = ThreadService::with_file_paths(state.database.clone(), state.config.paths.clone());
    match service.get_thread(&id)? {
        Some(thread) => Ok(Json(thread)),
        None => Err(ApiError::NotFound(format!("thread {id} not found"))),
    }
}

pub(crate) async fn download_thread(
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

pub(crate) async fn create_thread(
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

pub(crate) async fn create_post(
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
        Ok(mut post) => {
            // Calculate thread hash for synchronization
            // Get all posts in thread to calculate the hash
            let service_with_paths = ThreadService::with_file_paths(
                state.database.clone(),
                state.config.paths.clone()
            );
            if let Ok(Some(thread_details)) = service_with_paths.get_thread(&thread_id) {
                let thread_hash = crate::threading::calculate_thread_hash(&thread_details.posts);
                post.thread_hash = Some(thread_hash);
            }

            // Broadcast the post update with thread hash for synchronization
            if let Err(err) = state.network.publish_post_update(post.clone()).await {
                tracing::warn!(
                    error = ?err,
                    thread_id = %post.thread_id,
                    post_id = %post.id,
                    "failed to publish post update over network"
                );
            }

            // Re-announce the thread so peers can discover it (with updated post_count and hash)
            // This allows transitive discovery: if peer B adds to a thread, peer C who isn't subscribed
            // yet can discover the thread exists
            let service_with_paths = ThreadService::with_file_paths(
                state.database.clone(),
                state.config.paths.clone()
            );
            if let Ok(Some(thread_details)) = service_with_paths.get_thread(&thread_id) {
                // Re-announce thread with updated metadata
                if let Err(err) = state.network.publish_thread_announcement(
                    thread_details,
                    &state.identity.gpg_fingerprint
                ).await {
                    tracing::warn!(
                        error = ?err,
                        thread_id = %thread_id,
                        "failed to re-announce thread after new post"
                    );
                }
            }

            // Clear thread_hash before returning to client (they don't need it)
            post.thread_hash = None;

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

pub(crate) async fn list_recent_posts(
    State(state): State<AppState>,
    Query(params): Query<RecentPostsParams>,
) -> ApiResult<RecentPostsResponse> {
    let limit = params.limit.unwrap_or(50);

    let service = ThreadService::with_file_paths(
        state.database.clone(),
        state.config.paths.clone(),
    );

    let file_service = FileService::new(
        state.database.clone(),
        state.config.paths.clone(),
        state.config.file.clone(),
        state.blobs.clone(),
    );

    let post_records = state.database.with_repositories(|repos| {
        repos.posts().list_recent(limit)
    }).map_err(ApiError::Internal)?;

    let mut recent_posts = Vec::new();

    for post_record in post_records {
        // Get thread title
        let thread_title = state.database.with_repositories(|repos| {
            repos.threads().get(&post_record.thread_id)
                .map(|t| t.map(|thread| thread.title).unwrap_or_else(|| "Unknown Thread".to_string()))
        }).map_err(ApiError::Internal)?;

        // Get parent IDs
        let parent_post_ids = state.database.with_repositories(|repos| {
            repos.posts().parents_of(&post_record.id)
        }).map_err(ApiError::Internal)?;

        // Get files
        let file_views = file_service.list_post_files(&post_record.id)
            .map_err(ApiError::Internal)?;

        let files: Vec<FileResponse> = file_views.iter().map(|f| map_file_view(f.clone())).collect();

        // Convert to PostView
        // Parse metadata JSON if present
        let metadata = post_record.metadata.as_ref().and_then(|json_str| {
            serde_json::from_str::<crate::threading::PostMetadata>(json_str).ok()
        });

        let post_view = crate::threading::PostView {
            id: post_record.id.clone(),
            thread_id: post_record.thread_id.clone(),
            author_peer_id: post_record.author_peer_id.clone(),
            author_friendcode: post_record.author_friendcode.clone(),
            body: post_record.body.clone(),
            created_at: post_record.created_at.clone(),
            updated_at: post_record.updated_at.clone(),
            parent_post_ids,
            files: file_views,
            thread_hash: None,
            metadata,
        };

        recent_posts.push(RecentPostView {
            post: post_view,
            thread_title,
            files,
        });
    }

    Ok(Json(RecentPostsResponse { posts: recent_posts }))
}

pub(crate) async fn delete_thread(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    tracing::info!("delete_thread: Starting deletion for thread_id={}", id);

    // Get all file paths before deletion so we can clean them up
    let file_paths: Vec<std::path::PathBuf> = state.database.with_repositories(|repos| {
        tracing::info!("delete_thread: Calling FileRepository::list_for_thread");
        let files = FileRepository::list_for_thread(&repos.files(), &id)
            .map_err(|e| {
                tracing::error!("delete_thread: FileRepository::list_for_thread failed: {:?}", e);
                e
            })?;
        tracing::info!("delete_thread: Found {} files to delete", files.len());
        Ok(files.into_iter()
            .map(|f| std::path::PathBuf::from(f.path))
            .collect())
    })?;

    // Delete from database (cascades to posts, files, etc.)
    state.database.with_repositories(|repos| {
        repos.threads().delete(&id)
    })?;

    // Clean up actual files from disk
    for path in file_paths {
        if path.exists() {
            if let Err(e) = std::fs::remove_file(&path) {
                tracing::warn!("Failed to delete file {:?}: {}", path, e);
            } else {
                tracing::info!("Deleted file: {:?}", path);
            }
        }
    }

    Ok(StatusCode::OK)
}

pub(crate) async fn set_thread_ignored(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(payload): Json<SetIgnoredRequest>,
) -> Result<StatusCode, ApiError> {
    state.database.with_repositories(|repos| {
        repos.threads().set_ignored(&id, payload.ignored)
    })?;
    Ok(StatusCode::OK)
}

pub(crate) async fn import_thread_handler(
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
