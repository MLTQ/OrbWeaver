//! File-related ingest helpers.
//!
//! Handles `FileAnnouncement` arrivals (record metadata, decide whether to
//! download), the actual blob fetch via iroh-blobs, and small filesystem
//! helpers shared with the thread-snapshot path.

use crate::config::GraphchanPaths;
use crate::database::models::FileRecord;
use crate::database::repositories::{FileRepository, PostRepository};
use crate::database::Database;
use crate::network::events::FileAnnouncement;
use anyhow::{Context, Result};
use blake3::Hasher;
use iroh::endpoint::Endpoint;
use iroh_blobs::store::fs::FsStore;
use std::fs;
use std::sync::Arc;

/// Largest file size that auto-downloads on receiving a `FileAnnouncement`.
/// Larger files are recorded with `download_status = 'pending'` and require an
/// explicit `/files/{id}/download` POST from the user.
pub(super) const AUTO_DOWNLOAD_SIZE_LIMIT: i64 = 50 * 1024 * 1024;

pub(super) fn apply_file_announcement(
    database: &Database,
    paths: &GraphchanPaths,
    announcement: &FileAnnouncement,
) -> Result<bool> {
    tracing::debug!(
        file_id = %announcement.id,
        post_id = %announcement.post_id,
        "processing FileAnnouncement"
    );

    let existing_record =
        database.with_repositories(|repos| repos.files().get(&announcement.id))?;

    if let Some(existing) = &existing_record {
        let existing_path = paths.base.join(&existing.path);
        if existing_path.exists() {
            tracing::info!(
                file_id = %announcement.id,
                path = %existing_path.display(),
                "✅ file already exists locally, skipping announcement"
            );
            return Ok(false);
        } else {
            tracing::debug!(
                file_id = %announcement.id,
                path = %existing_path.display(),
                "file record exists but file missing on disk, will re-download"
            );
        }
    }

    let relative_path = format!("files/downloads/{}", announcement.id);
    let record = FileRecord {
        id: announcement.id.clone(),
        post_id: announcement.post_id.clone(),
        path: relative_path,
        original_name: announcement.original_name.clone(),
        mime: announcement.mime.clone(),
        blob_id: announcement.blob_id.clone(),
        size_bytes: announcement.size_bytes,
        checksum: announcement.checksum.clone(),
        ticket: announcement.ticket.as_ref().map(|t| t.to_string()),
        download_status: Some("pending".to_string()),
    };

    // Always persist the file record, even if post doesn't exist yet — the
    // post might arrive in a later message (ThreadSnapshot).
    let post_exists = database.with_repositories(|repos| {
        repos.files().upsert(&record)?;
        tracing::debug!(file_id = %announcement.id, "file record upserted");
        Ok(repos.posts().get(&announcement.post_id)?.is_some())
    })?;

    if !post_exists {
        tracing::info!(
            file_id = %announcement.id,
            post_id = %announcement.post_id,
            "💾 saved file record, but post doesn't exist yet - will download when post arrives"
        );
        return Ok(false);
    }

    let needs_fetch = file_needs_download(paths, &record)?;
    tracing::debug!(
        file_id = %announcement.id,
        needs_fetch = %needs_fetch,
        size_bytes = ?record.size_bytes,
        "checked if download needed"
    );

    if needs_fetch {
        if let Some(size) = record.size_bytes {
            if size > AUTO_DOWNLOAD_SIZE_LIMIT {
                tracing::info!(
                    file_id = %announcement.id,
                    size_mb = size / (1024 * 1024),
                    "⏸️ file exceeds auto-download limit ({}MB), marked as pending for manual download",
                    AUTO_DOWNLOAD_SIZE_LIMIT / (1024 * 1024)
                );
                ensure_download_directory(paths)?;
                return Ok(false);
            }
        } else {
            tracing::warn!(
                file_id = %announcement.id,
                "no size information available, allowing auto-download"
            );
        }
        ensure_download_directory(paths)?;
    }
    Ok(needs_fetch)
}

pub(super) fn ensure_download_directory(paths: &GraphchanPaths) -> Result<()> {
    if !paths.downloads_dir.exists() {
        fs::create_dir_all(&paths.downloads_dir)?;
    }
    Ok(())
}

pub(super) fn file_needs_download(paths: &GraphchanPaths, record: &FileRecord) -> Result<bool> {
    let absolute = paths.base.join(&record.path);
    if !absolute.exists() {
        return Ok(true);
    }
    if let Some(expected) = record.size_bytes {
        let actual = absolute.metadata()?.len() as i64;
        if actual != expected {
            return Ok(true);
        }
    }
    if let Some(expected_checksum) = &record.checksum {
        let data = fs::read(&absolute)?;
        let mut hasher = Hasher::new();
        hasher.update(&data);
        let hash = hasher.finalize();
        let actual = format!("blake3:{}", hash.to_hex());
        if &actual != expected_checksum {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(super) async fn download_blob(
    database: &Database,
    paths: &GraphchanPaths,
    announcement: &FileAnnouncement,
    blob_store: FsStore,
    endpoint: Arc<Endpoint>,
) -> Result<()> {
    let ticket = announcement
        .ticket
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("no ticket in announcement"))?;

    tracing::info!(
        file_id = %announcement.id,
        hash = %ticket.hash().fmt_short(),
        "downloading blob via iroh-blobs"
    );

    let hash = ticket.hash();

    let has_blob = blob_store
        .has(hash)
        .await
        .context("failed to check blob existence")?;

    if !has_blob {
        tracing::info!(
            file_id = %announcement.id,
            hash = %hash.fmt_short(),
            peer = %ticket.addr().id.fmt_short(),
            "blob not in local store - downloading from peer"
        );

        // Set status to 'downloading'
        let db_clone = database.clone();
        let file_id = announcement.id.clone();
        let _ = tokio::task::spawn_blocking(move || -> Result<()> {
            db_clone.with_repositories(|repos| {
                if let Ok(Some(mut record)) = repos.files().get(&file_id) {
                    record.download_status = Some("downloading".to_string());
                    let _ = repos.files().upsert(&record);
                }
                Ok(())
            })
        })
        .await;

        let downloader = blob_store.downloader(&endpoint);
        let download_result = downloader.download(hash, Some(ticket.addr().id)).await;

        match download_result {
            Ok(_) => {
                tracing::info!(
                    file_id = %announcement.id,
                    hash = %hash.fmt_short(),
                    "✅ blob downloaded successfully from peer"
                );
            }
            Err(err) => {
                tracing::warn!(
                    file_id = %announcement.id,
                    hash = %hash.fmt_short(),
                    error = ?err,
                    "⚠️  failed to download blob from peer"
                );

                let db_clone = database.clone();
                let file_id = announcement.id.clone();
                let _ = tokio::task::spawn_blocking(move || -> Result<()> {
                    db_clone.with_repositories(|repos| {
                        if let Ok(Some(mut record)) = repos.files().get(&file_id) {
                            record.download_status = Some("failed".to_string());
                            let _ = repos.files().upsert(&record);
                        }
                        Ok(())
                    })
                })
                .await;

                return Err(err.into());
            }
        }
    } else {
        tracing::info!(file_id = %announcement.id, "blob already in local store");
    }

    ensure_download_directory(paths)?;
    let relative_path = format!("files/downloads/{}", announcement.id);
    let absolute_path = paths.base.join(&relative_path);

    blob_store
        .export(hash, absolute_path.clone())
        .await
        .with_context(|| format!("failed to export blob to {}", absolute_path.display()))?;

    let data = fs::read(&absolute_path)
        .with_context(|| format!("failed to read exported file {}", absolute_path.display()))?;

    tracing::info!(
        file_id = %announcement.id,
        path = %absolute_path.display(),
        size = %data.len(),
        "blob exported to file"
    );

    let size = data.len() as i64;
    let mut hasher = Hasher::new();
    hasher.update(&data);
    let digest = hasher.finalize();
    let checksum = format!("blake3:{}", digest.to_hex());

    database.with_repositories(|repos| {
        if let Some(mut record) = repos.files().get(&announcement.id)? {
            record.path = relative_path;
            record.size_bytes = Some(size);
            record.checksum = Some(checksum);
            record.download_status = Some("available".to_string());
            repos.files().upsert(&record)?;
            tracing::info!(file_id = %announcement.id, "✅ blob downloaded and saved successfully");
        }
        Ok(())
    })?;

    Ok(())
}
