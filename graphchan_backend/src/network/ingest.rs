use crate::config::GraphchanPaths;
use crate::database::models::{FileRecord, PostRecord, ThreadRecord};
use crate::database::repositories::{FileRepository, PostRepository, ThreadRepository};
use crate::database::Database;
use crate::network::events::{
    EventPayload, FileAnnouncement, FileChunk, FileRequest, InboundGossip, NetworkEvent,
};
use crate::threading::{PostView, ThreadDetails};
use anyhow::Result;
use blake3::Hasher;
use iroh_blobs::store::fs::FsStore;
use std::fs;
use tokio::sync::mpsc::{Receiver, Sender};

pub async fn run_ingest_loop(
    database: Database,
    paths: GraphchanPaths,
    publisher: Sender<NetworkEvent>,
    mut rx: Receiver<InboundGossip>,
    blobs: FsStore,
) {
    let _ = &blobs;
    tracing::info!("network ingest loop started");
    while let Some(message) = rx.recv().await {
        let peer = message.peer_id.clone();
        if let Err(err) =
            handle_message(&database, &paths, &publisher, peer.clone(), message.payload)
        {
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
) -> Result<()> {
    match payload {
        EventPayload::ThreadSnapshot(snapshot) => apply_thread_snapshot(database, snapshot),
        EventPayload::PostUpdate(post) => apply_post_update(database, post),
        EventPayload::FileAvailable(announcement) => {
            let fetch_needed = apply_file_announcement(database, paths, &announcement)?;
            if fetch_needed {
                let request_payload = EventPayload::FileRequest(FileRequest {
                    file_id: announcement.id.clone(),
                });
                if let Some(peer_id) = peer_id.clone() {
                    let sender = publisher.clone();
                    let payload = request_payload.clone();
                    tokio::spawn(async move {
                        if let Err(err) =
                            sender.send(NetworkEvent::Direct { peer_id, payload }).await
                        {
                            tracing::warn!(error = ?err, "failed to enqueue direct file request");
                        }
                    });
                }

                let sender = publisher.clone();
                tokio::spawn(async move {
                    if let Err(err) = sender.send(NetworkEvent::Broadcast(request_payload)).await {
                        tracing::warn!(error = ?err, "failed to broadcast file request");
                    }
                });
            }
            Ok(())
        }
        EventPayload::FileRequest(request) => {
            if let Some(peer_id) = peer_id {
                respond_with_file_chunk(database, paths, publisher, &peer_id, request)?;
            }
            Ok(())
        }
        EventPayload::FileChunk(chunk) => apply_file_chunk(database, paths, chunk),
    }
}

fn apply_thread_snapshot(database: &Database, snapshot: ThreadDetails) -> Result<()> {
    let thread = snapshot.thread;
    let posts = snapshot.posts;
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
        Ok(())
    })
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

    let should_persist = database.with_repositories(|repos| {
        if repos.posts().get(&announcement.post_id)?.is_none() {
            tracing::debug!(
                file_id = %announcement.id,
                post_id = %announcement.post_id,
                "skipping file announcement because post is unknown"
            );
            return Ok(false);
        }
        repos.files().upsert(&record)?;
        Ok(true)
    })?;

    if !should_persist {
        return Ok(false);
    }

    let needs_fetch = file_needs_download(paths, &record)?;
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
        tracing::debug!(file_id = %request.file_id, path = %absolute.display(), "requested file missing locally");
        return Ok(());
    }
    let data = fs::read(&absolute)?;
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
    fs::write(&absolute, &chunk.data)?;

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
        tracing::warn!(file_id = %chunk.file_id, "file chunk arrived without prior announcement; discarding");
        let _ = fs::remove_file(&absolute);
        return Ok(());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::GraphchanPaths;
    use crate::database::models::{PostRecord, ThreadRecord};
    use crate::database::repositories::{PostRepository, ThreadRepository};
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

        let ingest_db = database.clone();
        let ingest_paths = paths.clone();
        let ingest_publisher = publisher_tx.clone();
        let handle = tokio::spawn(async move {
            run_ingest_loop(ingest_db, ingest_paths, ingest_publisher, inbound_rx, blob_store).await;
        });

        let secret = SecretKey::from_bytes(&[9u8; 32]);
        let hash = Hash::from_bytes([1u8; 32]);
        let blob_hex = hash.to_hex().to_string();
        let ticket = BlobTicket::new(EndpointAddr::new(secret.public()), hash, BlobFormat::Raw);
        let ticket_string = ticket.to_string();

        let announcement = FileAnnouncement {
            id: "file-1".into(),
            post_id: "post-1".into(),
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
