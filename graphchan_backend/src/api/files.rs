use super::{AppState, ApiError, FileResponse, map_file_view};
use crate::database::repositories::FileRepository;
use crate::files::{FileService, FileView, SaveFileInput};
use crate::network::FileAnnouncement;
use anyhow::{Context, Result};
use axum::body::Body;
use axum::extract::{Multipart, Path, Query, State};
use axum::http::{
    header::{CONTENT_DISPOSITION, CONTENT_LENGTH, CONTENT_TYPE},
    HeaderValue, StatusCode,
};
use axum::response::{IntoResponse, Response};
use axum::Json;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use iroh_blobs::{BlobFormat, Hash};
use serde::{Deserialize, Serialize};
use std::str::FromStr;
use tokio::fs::File as TokioFile;
use tokio_util::io::ReaderStream;

use super::ApiResult;
use crate::threading::ThreadService;

#[derive(Debug, Deserialize)]
pub(crate) struct ListFilesParams {
    #[serde(default)]
    missing_only: Option<bool>,
    #[serde(default)]
    mime: Option<String>,
}

#[derive(Serialize)]
pub(crate) struct TriggerDownloadResponse {
    status: String,
    message: String,
}

pub(crate) async fn list_post_files(
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

pub(crate) async fn upload_post_file(
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
                thread_id: thread_id.clone(),
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
                size_bytes = ?announcement.size_bytes,
                size_mb = announcement.size_bytes.map(|s| s / (1024 * 1024)),
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

pub(crate) async fn download_file(
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

pub(crate) async fn trigger_file_download(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<TriggerDownloadResponse>, ApiError> {
    use crate::database::repositories::FileRepository;

    // Get the file record with ticket information
    let file_record = state.database.with_repositories(|repos| {
        FileRepository::get(&repos.files(), &id)
    }).map_err(ApiError::Internal)?;

    let Some(record) = file_record else {
        return Err(ApiError::NotFound(format!("file {id} not found")));
    };

    // Check if file has a ticket for download
    let ticket_str = record.ticket.clone()
        .ok_or_else(|| ApiError::BadRequest("File has no download ticket available".to_string()))?;

    // Parse the ticket
    let ticket = BlobTicket::from_str(&ticket_str)
        .map_err(|e| ApiError::BadRequest(format!("Invalid ticket: {}", e)))?;

    // Set status to 'downloading' immediately
    state.database.with_repositories(|repos| {
        let mut updated_record = record.clone();
        updated_record.download_status = Some("downloading".to_string());
        FileRepository::upsert(&repos.files(), &updated_record)
    }).map_err(ApiError::Internal)?;

    // Spawn background task to download the file
    let db = state.database.clone();
    let paths = state.config.paths.clone();
    let blobs = state.blobs.clone();
    let endpoint = state.network.endpoint();
    let file_id = id.clone();

    tokio::spawn(async move {
        let hash = ticket.hash();

        tracing::info!(
            file_id = %file_id,
            hash = %hash.fmt_short(),
            "manual download triggered via API"
        );

        // Download the blob
        let download_result = async {
            // Check if blob exists
            let has_blob = blobs.has(hash).await
                .context("failed to check blob existence")?;

            if !has_blob {
                // Download from peer
                let downloader = blobs.downloader(&endpoint);
                downloader
                    .download(hash, Some(ticket.addr().id))
                    .await
                    .context("failed to download blob")?;
            }

            // Export to file
            let relative_path = format!("files/downloads/{}", file_id);
            let absolute_path = paths.base.join(&relative_path);

            // Ensure directory exists
            if let Some(parent) = absolute_path.parent() {
                tokio::fs::create_dir_all(parent).await
                    .context("failed to create download directory")?;
            }

            blobs.export(hash, absolute_path.clone()).await
                .context("failed to export blob")?;

            // Read and verify
            let data = tokio::fs::read(&absolute_path).await
                .context("failed to read exported file")?;

            let size = data.len() as i64;
            let mut hasher = blake3::Hasher::new();
            hasher.update(&data);
            let digest = hasher.finalize();
            let checksum = format!("blake3:{}", digest.to_hex());

            // Update database
            db.with_repositories(|repos| {
                use crate::database::repositories::FileRepository;
                if let Ok(Some(mut rec)) = FileRepository::get(&repos.files(), &file_id) {
                    rec.path = relative_path;
                    rec.size_bytes = Some(size);
                    rec.checksum = Some(checksum);
                    rec.download_status = Some("available".to_string());
                    let _ = FileRepository::upsert(&repos.files(), &rec);
                }
                Ok(())
            })?;

            tracing::info!(file_id = %file_id, "✅ manual download completed successfully");
            Ok::<(), anyhow::Error>(())
        }.await;

        // Update status to failed if download failed
        if let Err(e) = download_result {
            tracing::warn!(file_id = %file_id, error = ?e, "manual download failed");
            let _ = db.with_repositories(|repos| {
                use crate::database::repositories::FileRepository;
                if let Ok(Some(mut rec)) = FileRepository::get(&repos.files(), &file_id) {
                    rec.download_status = Some("failed".to_string());
                    let _ = FileRepository::upsert(&repos.files(), &rec);
                }
                Ok(())
            });
        }
    });

    Ok(Json(TriggerDownloadResponse {
        status: "downloading".to_string(),
        message: format!("Download started for file {}", id),
    }))
}

pub(crate) async fn get_blob(
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
