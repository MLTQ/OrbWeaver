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
        EventPayload::ThreadSnapshot(snapshot) => apply_thread_snapshot(database, paths, publisher, snapshot, blobs, endpoint),
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
        let thread_record = ThreadRecord {
            id: thread.id.clone(),
            title: thread.title.clone(),
            creator_peer_id: thread.creator_peer_id.clone(),
            created_at: thread.created_at.clone(),
            pinned: thread.pinned,
        };
        repos.threads().upsert(&thread_record)?;

        let posts_repo = repos.posts();
        for post in posts {
            upsert_post(&posts_repo, &post)?;
        }

        // Ingest peers
        let peers_repo = repos.peers();
        for peer in snapshot.peers {
            let record = crate::database::models::PeerRecord {
                id: peer.id,
                alias: peer.alias,
                username: peer.username,
                bio: peer.bio,
                friendcode: peer.friendcode,
                iroh_peer_id: peer.iroh_peer_id,
                gpg_fingerprint: peer.gpg_fingerprint,
                last_seen: peer.last_seen,
                avatar_file_id: peer.avatar_file_id,
                trust_state: peer.trust_state,
            };
            peers_repo.upsert(&record)?;
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
