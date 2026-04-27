//! Re-download a thread snapshot when the local hash diverges from a peer's.
//!
//! Triggered by `ResyncRequest`s emitted from `apply_post_update` when the
//! incoming `thread_hash` doesn't match what we have locally. Pulls the full
//! `ThreadDetails` blob via iroh-blobs and replays it through the same
//! `apply_thread_snapshot` path used by direct downloads.

use crate::config::GraphchanPaths;
use crate::database::Database;
use crate::network::events::NetworkEvent;
use anyhow::{Context, Result};
use iroh::endpoint::Endpoint;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use std::sync::Arc;
use tokio::sync::mpsc::Sender;

pub(super) async fn download_thread_snapshot_blob(
    database: &Database,
    paths: &GraphchanPaths,
    publisher: &Sender<NetworkEvent>,
    ticket: BlobTicket,
    blob_store: FsStore,
    endpoint: Arc<Endpoint>,
) -> Result<()> {
    let hash = ticket.hash();

    tracing::info!(
        hash = %hash.fmt_short(),
        peer = %ticket.addr().id.fmt_short(),
        "downloading thread snapshot blob via iroh-blobs"
    );

    let has_blob = blob_store
        .has(hash)
        .await
        .context("failed to check blob existence")?;

    if !has_blob {
        let downloader = blob_store.downloader(&endpoint);
        downloader
            .download(hash, Some(ticket.addr().id))
            .await
            .context("failed to download thread snapshot blob")?;

        tracing::info!(
            hash = %hash.fmt_short(),
            "✅ thread snapshot blob downloaded successfully"
        );
    } else {
        tracing::info!("thread snapshot blob already in local store");
    }

    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("thread_snapshot_{}.json", hash.fmt_short()));

    blob_store
        .export(hash, temp_path.clone())
        .await
        .context("failed to export thread snapshot blob")?;

    let blob_bytes =
        std::fs::read(&temp_path).context("failed to read exported thread snapshot")?;

    let _ = std::fs::remove_file(&temp_path);

    let snapshot: crate::threading::ThreadDetails = serde_json::from_slice(&blob_bytes)
        .context("failed to deserialize thread snapshot from blob")?;

    tracing::info!(
        thread_id = %snapshot.thread.id,
        post_count = snapshot.posts.len(),
        "deserialized thread snapshot from blob - applying to database"
    );

    super::apply_thread_snapshot(database, paths, publisher, snapshot, &blob_store, &endpoint)
}
