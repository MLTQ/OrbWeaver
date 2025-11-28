use crate::config::GraphchanPaths;
use crate::database::models::{FileRecord, PostRecord, ThreadRecord};
use crate::database::repositories::{FileRepository, PeerRepository, PostRepository, ThreadRepository};
use crate::database::Database;
use crate::network::events::{
    EventPayload, FileAnnouncement, FileChunk, FileRequest, InboundGossip, NetworkEvent,
    ProfileUpdate,
};
use crate::peers::PeerService;
use crate::threading::{PostView, ThreadDetails};
use anyhow::{Context, Result};
use blake3::Hasher;
use iroh::endpoint::Endpoint;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use std::fs;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};

pub async fn run_ingest_loop(
    database: Database,
    paths: GraphchanPaths,
    publisher: Sender<NetworkEvent>,
    mut rx: Receiver<InboundGossip>,
    blobs: FsStore,
    endpoint: Arc<Endpoint>,
) {
    tracing::info!("network ingest loop started");
    while let Some(message) = rx.recv().await {
        let peer = message.peer_id.clone();
        if let Err(err) = handle_message(
            &database,
            &paths,
            &publisher,
            peer.clone(),
            message.payload,
            &blobs,
            &endpoint,
        ) {
            tracing::warn!(error = ?err, ?peer, "failed to apply inbound gossip payload");
        }
    }
    tracing::info!("network ingest loop shutting down");
}

fn handle_message(
    database: &Database,
    paths: &GraphchanPaths,
    publisher: &Sender<NetworkEvent>,
    peer_id: Option<String>,
    payload: EventPayload,
    blobs: &FsStore,
    endpoint: &Arc<Endpoint>,
) -> Result<()> {
    match payload {
        EventPayload::ThreadAnnouncement(announcement) => {
            tracing::info!(
                thread_id = %announcement.thread_id,
                title = %announcement.title,
                post_count = announcement.post_count,
                announcer = %announcement.announcer_peer_id,
                "📢 received thread announcement (will download on-demand)"
            );
            apply_thread_announcement(database, announcement)
        }
        EventPayload::PostUpdate(post) => apply_post_update(database, post),
        EventPayload::FileAvailable(announcement) => {
            let fetch_needed = apply_file_announcement(database, paths, &announcement)?;
            if fetch_needed && announcement.ticket.is_some() {
                tracing::info!(
                    file_id = %announcement.id,
                    post_id = %announcement.post_id,
                    "📥 file needed - downloading via blob ticket"
                );
                // Use iroh-blobs to download directly
                let db = database.clone();
                let p = paths.clone();
                let ann = announcement.clone();
                let blob_store = blobs.clone();
                let ep = endpoint.clone();
                tokio::spawn(async move {
                    if let Err(err) = download_blob(&db, &p, &ann, blob_store, ep).await {
                        tracing::warn!(error = ?err, file_id = %ann.id, "failed to download blob");
                    }
                });
            } else if fetch_needed {
                tracing::warn!(
                    file_id = %announcement.id,
                    "file needed but no blob ticket available"
                );
            }
            Ok(())
        }
        EventPayload::FileRequest(request) => {
            if let Some(peer_id) = peer_id {
                tracing::info!(file_id = %request.file_id, peer = %peer_id, "📤 received file request, preparing to send chunk");
                respond_with_file_chunk(database, paths, publisher, &peer_id, request)?;
            } else {
                tracing::debug!(file_id = %request.file_id, "received file request without peer_id");
            }
            Ok(())
        }
        EventPayload::FileChunk(chunk) => {
            tracing::info!(file_id = %chunk.file_id, size = %chunk.data.len(), "📦 received file chunk");
            apply_file_chunk(database, paths, chunk)
        }
        EventPayload::ProfileUpdate(update) => apply_profile_update(database, update),
    }
}

fn apply_profile_update(database: &Database, update: ProfileUpdate) -> Result<()> {
    let service = PeerService::new(database.clone());
    service.update_profile(&update.peer_id, update.avatar_file_id, update.username, update.bio)?;
    Ok(())
}

/// Stores just the announcement metadata - the full thread will be downloaded on-demand
fn apply_thread_announcement(
    database: &Database,
    announcement: crate::network::events::ThreadAnnouncement,
) -> Result<()> {
    use crate::database::models::{PeerRecord, ThreadRecord, PostRecord};

    database.with_repositories(|repos| {
        // Check if we already have this thread
        let existing = repos.threads().get(&announcement.thread_id)?;
        if let Some(existing_thread) = existing {
            // Compare hashes to detect if we need to sync
            match (&existing_thread.thread_hash, &announcement.thread_hash) {
                (Some(local_hash), remote_hash) if local_hash == remote_hash => {
                    tracing::debug!(
                        thread_id = %announcement.thread_id,
                        hash = %local_hash,
                        "thread hash matches - already in sync"
                    );
                    return Ok(());
                }
                (Some(local_hash), remote_hash) => {
                    tracing::info!(
                        thread_id = %announcement.thread_id,
                        local_hash = %local_hash,
                        remote_hash = %remote_hash,
                        "thread hash mismatch - will re-sync on next view"
                    );
                    // Update the thread record with new hash and ticket
                    // The actual sync will happen when user views the thread
                    let updated_thread = ThreadRecord {
                        id: announcement.thread_id.clone(),
                        title: announcement.title.clone(),
                        creator_peer_id: Some(announcement.creator_peer_id.clone()),
                        created_at: announcement.created_at.clone(),
                        pinned: existing_thread.pinned,
                        thread_hash: Some(announcement.thread_hash.clone()),
                    };
                    repos.threads().upsert(&updated_thread)?;

                    // Update the ticket for downloading
                    let ticket_str = announcement.ticket.to_string();
                    repos.conn().execute(
                        "INSERT OR REPLACE INTO thread_tickets (thread_id, ticket) VALUES (?, ?)",
                        rusqlite::params![announcement.thread_id, ticket_str],
                    )?;
                    return Ok(());
                }
                (None, _) => {
                    tracing::debug!(
                        thread_id = %announcement.thread_id,
                        "thread exists but has no hash - treating as out of sync"
                    );
                    // Fall through to update the thread
                }
            }
        }

        // Create stub peer for creator if needed
        let peers_repo = repos.peers();
        if peers_repo.get(&announcement.creator_peer_id)?.is_none() {
            let stub_peer = PeerRecord {
                id: announcement.creator_peer_id.clone(),
                alias: None,
                username: Some(format!("Unknown ({})", &announcement.creator_peer_id[..8])),
                bio: None,
                friendcode: None,
                iroh_peer_id: None,
                gpg_fingerprint: Some(announcement.creator_peer_id.clone()),
                last_seen: None,
                avatar_file_id: None,
                trust_state: "unknown".into(),
            };
            peers_repo.upsert(&stub_peer)?;
        }

        // Create thread entry with minimal info
        let thread_record = ThreadRecord {
            id: announcement.thread_id.clone(),
            title: announcement.title.clone(),
            creator_peer_id: Some(announcement.creator_peer_id.clone()),
            created_at: announcement.created_at.clone(),
            pinned: false,
            thread_hash: Some(announcement.thread_hash.clone()),
        };
        repos.threads().upsert(&thread_record)?;

        // Create a placeholder OP post with the preview
        // This lets the thread show up in catalog with preview text
        let op_post = PostRecord {
            id: format!("{}-preview", announcement.thread_id),
            thread_id: announcement.thread_id.clone(),
            author_peer_id: Some(announcement.creator_peer_id.clone()),
            body: format!("{}...", announcement.preview),
            created_at: announcement.created_at.clone(),
            updated_at: None,
        };
        repos.posts().upsert(&op_post)?;

        // Store the BlobTicket for later download
        let ticket_str = announcement.ticket.to_string();
        repos.conn().execute(
            "INSERT OR REPLACE INTO thread_tickets (thread_id, ticket) VALUES (?, ?)",
            rusqlite::params![announcement.thread_id, ticket_str],
        )?;

        tracing::info!(
            thread_id = %announcement.thread_id,
            title = %announcement.title,
            post_count = announcement.post_count,
            "✅ saved thread announcement with ticket (full thread available on-demand)"
        );

        Ok(())
    })
}

fn apply_thread_snapshot(
    database: &Database,
    paths: &GraphchanPaths,
    _publisher: &Sender<NetworkEvent>,
    snapshot: ThreadDetails,
    blobs: &FsStore,
    endpoint: &Arc<Endpoint>,
) -> Result<()> {
    let thread = snapshot.thread;
    let posts = snapshot.posts;
    let post_ids: Vec<String> = posts.iter().map(|p| p.id.clone()).collect();

    database.with_repositories(|repos| {
        // First, ingest all peers from the snapshot
        let peers_repo = repos.peers();
        for peer in &snapshot.peers {
            let record = crate::database::models::PeerRecord {
                id: peer.id.clone(),
                alias: peer.alias.clone(),
                username: peer.username.clone(),
                bio: peer.bio.clone(),
                friendcode: peer.friendcode.clone(),
                iroh_peer_id: peer.iroh_peer_id.clone(),
                gpg_fingerprint: peer.gpg_fingerprint.clone(),
                last_seen: peer.last_seen.clone(),
                avatar_file_id: peer.avatar_file_id.clone(),
                trust_state: peer.trust_state.clone(),
            };
            peers_repo.upsert(&record)?;
        }

        // Collect all author peer IDs from posts
        let mut all_author_ids = std::collections::HashSet::new();
        if let Some(creator_id) = &thread.creator_peer_id {
            all_author_ids.insert(creator_id.clone());
        }
        for post in &posts {
            if let Some(author_id) = &post.author_peer_id {
                all_author_ids.insert(author_id.clone());
            }
        }

        // Create stub peer records for any authors not in the snapshot
        for author_id in all_author_ids {
            if peers_repo.get(&author_id)?.is_none() {
                tracing::info!(peer_id = %author_id, "creating stub peer for unknown author in thread snapshot");
                let stub_peer = crate::database::models::PeerRecord {
                    id: author_id.clone(),
                    alias: None,
                    username: Some(format!("Unknown ({})", &author_id[..8])),
                    bio: None,
                    friendcode: None,
                    iroh_peer_id: None,
                    gpg_fingerprint: Some(author_id.clone()),
                    last_seen: None,
                    avatar_file_id: None,
                    trust_state: "unknown".into(),
                };
                peers_repo.upsert(&stub_peer)?;
            }
        }

        // Now upsert thread and posts
        // Calculate hash from the posts we're applying
        let thread_hash = crate::threading::calculate_thread_hash(&posts);

        let thread_record = ThreadRecord {
            id: thread.id.clone(),
            title: thread.title.clone(),
            creator_peer_id: thread.creator_peer_id.clone(),
            created_at: thread.created_at.clone(),
            pinned: thread.pinned,
            thread_hash: Some(thread_hash),
        };
        repos.threads().upsert(&thread_record)?;

        let posts_repo = repos.posts();
        let files_repo = repos.files();

        // Delete the preview placeholder post if it exists
        let preview_post_id = format!("{}-preview", thread.id);
        if let Err(err) = repos.conn().execute(
            "DELETE FROM posts WHERE id = ?1",
            rusqlite::params![preview_post_id],
        ) {
            tracing::warn!(error = ?err, post_id = %preview_post_id, "failed to delete preview post");
        }

        for post in &posts {
            upsert_post(&posts_repo, &post)?;

            // Also save file metadata from the post
            for file in &post.files {
                let file_record = crate::database::models::FileRecord {
                    id: file.id.clone(),
                    post_id: file.post_id.clone(),
                    path: file.path.clone(),
                    original_name: file.original_name.clone(),
                    mime: file.mime.clone(),
                    blob_id: file.blob_id.clone(),
                    size_bytes: file.size_bytes,
                    checksum: file.checksum.clone(),
                    ticket: file.ticket.clone(),
                };
                files_repo.upsert(&file_record)?;
            }
        }

        Ok(())
    })?;

    // After creating posts, check for any files that need downloading
    // (Files might have arrived before the posts existed)
    for post_id in post_ids {
        let files = database.with_repositories(|repos| {
            repos.files().list_for_post(&post_id)
        })?;

        for file in files {
            let needs_fetch = file_needs_download(paths, &file)?;
            if needs_fetch && file.ticket.is_some() {
                tracing::info!(
                    file_id = %file.id,
                    post_id = %post_id,
                    "📥 post now exists - downloading pending blob"
                );
                // Convert the file record into a FileAnnouncement for blob download
                let announcement = FileAnnouncement {
                    id: file.id.clone(),
                    post_id: file.post_id.clone(),
                    thread_id: thread.id.clone(),
                    original_name: file.original_name.clone(),
                    mime: file.mime.clone(),
                    size_bytes: file.size_bytes,
                    checksum: file.checksum.clone(),
                    blob_id: file.blob_id.clone(),
                    ticket: file.ticket.as_ref().and_then(|t| {
                        use std::str::FromStr;
                        iroh_blobs::ticket::BlobTicket::from_str(t).ok()
                    }),
                };

                let db = database.clone();
                let p = paths.clone();
                let blob_store = blobs.clone();
                let ep = endpoint.clone();
                tokio::spawn(async move {
                    if let Err(err) = download_blob(&db, &p, &announcement, blob_store, ep).await {
                        tracing::warn!(error = ?err, file_id = %announcement.id, "failed to download pending blob");
                    }
                });
            }
        }
    }

    Ok(())
}

fn apply_post_update(database: &Database, post: PostView) -> Result<()> {
    database.with_repositories(|repos| {
        if repos.threads().get(&post.thread_id)?.is_none() {
            tracing::debug!(
                thread_id = %post.thread_id,
                post_id = %post.id,
                "skipping post update because thread is unknown"
            );
            return Ok(());
        }

        // Ensure the author peer exists - create stub if unknown
        if let Some(author_id) = &post.author_peer_id {
            let peers_repo = repos.peers();
            if peers_repo.get(author_id)?.is_none() {
                tracing::info!(peer_id = %author_id, "creating stub peer for unknown post author");
                let stub_peer = crate::database::models::PeerRecord {
                    id: author_id.clone(),
                    alias: None,
                    username: Some(format!("Unknown ({})", &author_id[..8])),
                    bio: None,
                    friendcode: None,
                    iroh_peer_id: None,
                    gpg_fingerprint: Some(author_id.clone()),
                    last_seen: None,
                    avatar_file_id: None,
                    trust_state: "unknown".into(),
                };
                peers_repo.upsert(&stub_peer)?;
            }
        }

        let posts_repo = repos.posts();
        upsert_post(&posts_repo, &post)?;
        Ok(())
    })
}

fn upsert_post<R>(repo: &R, post: &PostView) -> Result<()>
where
    R: PostRepository,
{
    let record = PostRecord {
        id: post.id.clone(),
        thread_id: post.thread_id.clone(),
        author_peer_id: post.author_peer_id.clone(),
        body: post.body.clone(),
        created_at: post.created_at.clone(),
        updated_at: post.updated_at.clone(),
    };
    repo.upsert(&record)?;
    repo.add_relationships(&record.id, &post.parent_post_ids)?;
    Ok(())
}

fn apply_file_announcement(
    database: &Database,
    paths: &GraphchanPaths,
    announcement: &FileAnnouncement,
) -> Result<bool> {
    tracing::debug!(
        file_id = %announcement.id,
        post_id = %announcement.post_id,
        "processing FileAnnouncement"
    );

    // Check if we already have this file locally (we're the uploader)
    let existing_record = database.with_repositories(|repos| {
        repos.files().get(&announcement.id)
    })?;

    if let Some(existing) = &existing_record {
        let existing_path = paths.base.join(&existing.path);
        if existing_path.exists() {
            tracing::info!(
                file_id = %announcement.id,
                path = %existing_path.display(),
                "✅ file already exists locally, skipping announcement"
            );
            return Ok(false); // We already have it, no need to fetch
        } else {
            tracing::debug!(
                file_id = %announcement.id,
                path = %existing_path.display(),
                "file record exists but file missing on disk, will re-download"
            );
        }
    }

    // We don't have it locally, create/update record for download
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
    };

    // Always persist the file record, even if post doesn't exist yet
    // The post might arrive in a later message (ThreadSnapshot)
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
        return Ok(false); // Don't download yet, wait for post
    }

    let needs_fetch = file_needs_download(paths, &record)?;
    tracing::debug!(file_id = %announcement.id, needs_fetch = %needs_fetch, "checked if download needed");
    if needs_fetch {
        ensure_download_directory(paths)?;
    }
    Ok(needs_fetch)
}

fn ensure_download_directory(paths: &GraphchanPaths) -> Result<()> {
    if !paths.downloads_dir.exists() {
        fs::create_dir_all(&paths.downloads_dir)?;
    }
    Ok(())
}

fn file_needs_download(paths: &GraphchanPaths, record: &FileRecord) -> Result<bool> {
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

fn respond_with_file_chunk(
    database: &Database,
    paths: &GraphchanPaths,
    publisher: &Sender<NetworkEvent>,
    peer_id: &str,
    request: FileRequest,
) -> Result<()> {
    let record = match database.with_repositories(|repos| repos.files().get(&request.file_id))? {
        Some(record) => record,
        None => {
            tracing::debug!(file_id = %request.file_id, "ignoring file request for unknown file");
            return Ok(());
        }
    };
    let absolute = paths.base.join(&record.path);
    if !absolute.exists() {
        tracing::warn!(file_id = %request.file_id, path = %absolute.display(), "⚠️  requested file missing locally");
        return Ok(());
    }
    let data = fs::read(&absolute)
        .with_context(|| format!("failed to read file for chunk: {}", absolute.display()))?;
    tracing::info!(file_id = %request.file_id, size = %data.len(), "sending file chunk");
    let chunk_payload = EventPayload::FileChunk(FileChunk {
        file_id: request.file_id,
        data,
        eof: true,
    });

    let direct_sender = publisher.clone();
    let direct_peer = peer_id.to_string();
    let direct_payload = chunk_payload.clone();
    tokio::spawn(async move {
        if let Err(err) = direct_sender
            .send(NetworkEvent::Direct {
                peer_id: direct_peer,
                payload: direct_payload,
            })
            .await
        {
            tracing::warn!(error = ?err, "failed to enqueue direct file chunk");
        }
    });

    let broadcast_sender = publisher.clone();
    tokio::spawn(async move {
        if let Err(err) = broadcast_sender
            .send(NetworkEvent::Broadcast(chunk_payload))
            .await
        {
            tracing::warn!(error = ?err, "failed to broadcast file chunk");
        }
    });
    Ok(())
}

fn apply_file_chunk(database: &Database, paths: &GraphchanPaths, chunk: FileChunk) -> Result<()> {
    if !chunk.eof {
        tracing::debug!(file_id = %chunk.file_id, "received non-eof chunk; treating as complete file");
    }
    ensure_download_directory(paths)?;
    let relative = format!("files/downloads/{}", chunk.file_id);
    let absolute = paths.base.join(&relative);

    fs::write(&absolute, &chunk.data)
        .with_context(|| format!("failed to write file chunk to {}", absolute.display()))?;
    tracing::debug!(file_id = %chunk.file_id, path = %absolute.display(), "wrote file chunk to disk");

    let mut hasher = Hasher::new();
    hasher.update(&chunk.data);
    let digest = hasher.finalize();
    let checksum = format!("blake3:{}", digest.to_hex());
    let blob_id = digest.to_hex().to_string();
    let size = chunk.data.len() as i64;

    let known = database.with_repositories(|repos| {
        if let Some(mut record) = repos.files().get(&chunk.file_id)? {
            record.path = relative.clone();
            record.size_bytes = Some(size);
            record.checksum = Some(checksum.clone());
            if record.blob_id.is_none() {
                record.blob_id = Some(blob_id.clone());
            }
            repos.files().upsert(&record)?;
            Ok(true)
        } else {
            Ok(false)
        }
    })?;

    if !known {
        tracing::warn!(file_id = %chunk.file_id, "⚠️  file chunk arrived without prior announcement; discarding");
        let _ = fs::remove_file(&absolute);
        return Ok(());
    }

    tracing::info!(file_id = %chunk.file_id, size_bytes = size, "✅ file downloaded and saved successfully");
    Ok(())
}

async fn download_thread_snapshot_blob(
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

    // Check if blob already exists
    let has_blob = blob_store.has(hash).await.context("failed to check blob existence")?;

    if !has_blob {
        // Download the blob from the peer
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

    // Read blob bytes - we need to export to a temporary file first
    let temp_dir = std::env::temp_dir();
    let temp_path = temp_dir.join(format!("thread_snapshot_{}.json", hash.fmt_short()));

    blob_store
        .export(hash, temp_path.clone())
        .await
        .context("failed to export thread snapshot blob")?;

    let blob_bytes = std::fs::read(&temp_path)
        .context("failed to read exported thread snapshot")?;

    // Clean up temp file
    let _ = std::fs::remove_file(&temp_path);

    // Deserialize the ThreadDetails
    let snapshot: crate::threading::ThreadDetails = serde_json::from_slice(&blob_bytes)
        .context("failed to deserialize thread snapshot from blob")?;

    tracing::info!(
        thread_id = %snapshot.thread.id,
        post_count = snapshot.posts.len(),
        "deserialized thread snapshot from blob - applying to database"
    );

    // Apply the snapshot using existing logic
    apply_thread_snapshot(database, paths, publisher, snapshot, &blob_store, &endpoint)
}

async fn download_blob(
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

    // Check if blob already exists in local store
    let has_blob = blob_store.has(hash).await.context("failed to check blob existence")?;

    if !has_blob {
        tracing::info!(
            file_id = %announcement.id,
            hash = %hash.fmt_short(),
            peer = %ticket.addr().id.fmt_short(),
            "blob not in local store - downloading from peer"
        );

        // Download the blob from the peer specified in the ticket
        let downloader = blob_store.downloader(&endpoint);
        let download_result = downloader
            .download(hash, Some(ticket.addr().id))
            .await;

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
                return Err(err.into());
            }
        }
    } else {
        tracing::info!(file_id = %announcement.id, "blob already in local store");
    }

    // Export blob to file
    ensure_download_directory(paths)?;
    let relative_path = format!("files/downloads/{}", announcement.id);
    let absolute_path = paths.base.join(&relative_path);

    // Read blob data and write to file - use export method
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

    // Update database record
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
            repos.files().upsert(&record)?;
            tracing::info!(file_id = %announcement.id, "✅ blob downloaded and saved successfully");
        }
        Ok(())
    })?;

    Ok(())
}

/// Public wrapper for applying a downloaded thread to the database.
/// This is called when a user manually downloads a thread on-demand.
pub async fn apply_thread_from_download(
    database: &Database,
    paths: &GraphchanPaths,
    network: &crate::network::NetworkHandle,
    thread_details: ThreadDetails,
    blobs: &FsStore,
) -> Result<()> {
    let thread_id = thread_details.thread.id.clone();

    // We don't need the publisher for on-demand downloads since we don't re-broadcast
    let (tx, _rx) = tokio::sync::mpsc::channel(1);
    let endpoint = network.endpoint();

    // Apply the thread to database
    apply_thread_snapshot(
        database,
        paths,
        &tx,
        thread_details,
        blobs,
        &endpoint,
    )?;

    // Subscribe to thread-specific topic to receive future PostUpdates and FileAnnouncements
    network.subscribe_to_thread(&thread_id).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GraphchanPaths;
    use crate::database::models::{PeerRecord, PostRecord, ThreadRecord};
    use crate::database::repositories::{PeerRepository, PostRepository, ThreadRepository};
    use crate::database::Database;
    use crate::utils::now_utc_iso;
    use iroh::SecretKey;
    use iroh_base::EndpointAddr;
    use iroh_blobs::{ticket::BlobTicket, BlobFormat, Hash};
    use rusqlite::Connection;
    use tempfile::tempdir;
    use tokio::sync::mpsc;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn file_announcement_persists_ticket_and_requests_fetch() {
        let temp = tempdir().expect("tempdir");
        let paths = GraphchanPaths::from_base_dir(temp.path()).expect("paths");
        let conn = Connection::open_in_memory().expect("db");
        let database = Database::from_connection(conn, true);
        database.ensure_migrations().expect("migrations");

        database
            .with_repositories(|repos| {
                repos.threads().create(&ThreadRecord {
                    id: "thread-1".into(),
                    title: "T".into(),
                    creator_peer_id: None,
                    created_at: now_utc_iso(),
                    pinned: false,
                    thread_hash: None,
                })?;
                repos.posts().create(&PostRecord {
                    id: "post-1".into(),
                    thread_id: "thread-1".into(),
                    author_peer_id: None,
                    body: "body".into(),
                    created_at: now_utc_iso(),
                    updated_at: None,
                })?;
                Ok(())
            })
            .expect("seed");

        let (publisher_tx, mut publisher_rx) = mpsc::channel(8);
        let (inbound_tx, inbound_rx) = mpsc::channel(1);
        let blob_store = FsStore::load(&paths.blobs_dir).await.expect("blob store");

        let secret = SecretKey::from_bytes(&[9u8; 32]);
        let endpoint = Arc::new(
            iroh::endpoint::Endpoint::builder()
                .secret_key(secret.clone())
                .bind()
                .await
                .expect("endpoint"),
        );

        let ingest_db = database.clone();
        let ingest_paths = paths.clone();
        let ingest_publisher = publisher_tx.clone();
        let ingest_endpoint = endpoint.clone();
        let handle = tokio::spawn(async move {
            run_ingest_loop(
                ingest_db,
                ingest_paths,
                ingest_publisher,
                inbound_rx,
                blob_store,
                ingest_endpoint,
            )
            .await;
        });

        let hash = Hash::from_bytes([1u8; 32]);
        let blob_hex = hash.to_hex().to_string();
        let ticket = BlobTicket::new(EndpointAddr::new(secret.public()), hash, BlobFormat::Raw);
        let ticket_string = ticket.to_string();

        let announcement = FileAnnouncement {
            id: "file-1".into(),
            post_id: "post-1".into(),
            thread_id: "thread-1".into(),
            original_name: Some("note.txt".into()),
            mime: Some("text/plain".into()),
            size_bytes: Some(4),
            checksum: Some(format!("blake3:{}", blob_hex)),
            blob_id: Some(blob_hex.clone()),
            ticket: Some(ticket.clone()),
        };

        inbound_tx
            .send(InboundGossip {
                peer_id: Some("peer-1".into()),
                payload: EventPayload::FileAvailable(announcement),
            })
            .await
            .expect("send announcement");

        let _ = timeout(Duration::from_secs(2), publisher_rx.recv())
            .await
            .expect("ingest did not publish request");

        drop(inbound_tx);
        drop(publisher_tx);

        handle.await.expect("ingest loop");

        let record = database
            .with_repositories(|repos| repos.files().get("file-1"))
            .expect("query")
            .expect("record");
        assert_eq!(record.ticket.as_deref(), Some(ticket_string.as_str()));
        assert_eq!(record.blob_id.as_deref(), Some(blob_hex.as_str()));
    }
}
