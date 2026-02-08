use crate::blocking::IpBlockChecker;
use crate::config::GraphchanPaths;
use crate::database::models::{FileRecord, PostRecord, ReactionRecord, ThreadRecord};
use crate::database::repositories::{FileRepository, PeerIpRepository, PeerRepository, PostRepository, ReactionRepository, ThreadRepository};
use crate::database::Database;
use crate::network::events::{
    EventPayload, FileAnnouncement, FileChunk, FileRequest, InboundGossip, NetworkEvent,
    ProfileUpdate, ReactionUpdate,
};
use crate::peers::PeerService;
use crate::threading::{PostView, ThreadDetails};
use anyhow::{Context, Result};
use blake3::Hasher;
use iroh::endpoint::Endpoint;
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::ticket::BlobTicket;
use rusqlite::OptionalExtension;
use std::collections::HashSet;
use std::fs;
use std::sync::Arc;
use tokio::sync::mpsc::{Receiver, Sender};
use tokio::sync::Mutex;

/// Request to resynchronize a thread due to detected hash mismatch
struct ResyncRequest {
    thread_id: String,
    ticket: BlobTicket,
}

pub async fn run_ingest_loop(
    database: Database,
    paths: GraphchanPaths,
    publisher: Sender<NetworkEvent>,
    mut rx: Receiver<InboundGossip>,
    blobs: FsStore,
    endpoint: Arc<Endpoint>,
    local_peer_id: String,
    ip_blocker: IpBlockChecker,
) {
    tracing::info!("network ingest loop started");

    // Cache of recently seen message IDs to prevent re-broadcast loops
    // Format: "post:{id}" or "thread:{id}" or "file:{id}" etc.
    let seen_messages: Arc<Mutex<HashSet<String>>> = Arc::new(Mutex::new(HashSet::new()));

    while let Some(message) = rx.recv().await {
        let peer = message.peer_id.clone();
        match handle_message(
            &database,
            &paths,
            &publisher,
            peer.clone(),
            message.payload,
            &blobs,
            &endpoint,
            &seen_messages,
            &local_peer_id,
            &ip_blocker,
        ).await {
            Ok(Some(resync_request)) => {
                // Spawn background task to re-download thread
                let db = database.clone();
                let p = paths.clone();
                let pub_clone = publisher.clone();
                let blobs_clone = blobs.clone();
                let ep = endpoint.clone();

                tokio::spawn(async move {
                    tracing::info!(
                        thread_id = %resync_request.thread_id,
                        "🔄 triggering automatic thread re-sync due to hash mismatch"
                    );
                    if let Err(err) = download_thread_snapshot_blob(
                        &db,
                        &p,
                        &pub_clone,
                        resync_request.ticket,
                        blobs_clone,
                        ep,
                    ).await {
                        tracing::warn!(
                            error = ?err,
                            thread_id = %resync_request.thread_id,
                            "failed to auto-resync thread"
                        );
                    } else {
                        tracing::info!(
                            thread_id = %resync_request.thread_id,
                            "✅ thread auto-resync completed successfully"
                        );
                    }
                });
            }
            Ok(None) => {
                // No resync needed
            }
            Err(err) => {
                tracing::warn!(error = ?err, ?peer, "failed to apply inbound gossip payload");
            }
        }
    }
    tracing::info!("network ingest loop shutting down");
}

/// Capture and store the IP address of a peer from the endpoint connection
///
/// TODO: Iroh 0.94's Endpoint API doesn't directly expose per-peer connection information.
/// Possible solutions:
/// 1. Use endpoint.magic_endpoint() to access underlying connection tracking
/// 2. Monitor connection events via endpoint event streams
/// 3. Track connections at a lower level (QUIC connection events)
/// 4. Wait for Iroh API updates that expose this information
///
/// For now, this is a stub. We'll need to implement proper IP tracking based on
/// the actual Iroh API capabilities.
async fn capture_peer_ip(
    _database: &Database,
    _endpoint: &Endpoint,
    peer_id_str: &str,
) -> Result<()> {
    tracing::trace!(
        peer_id = %peer_id_str,
        "IP capture not yet implemented for Iroh 0.94 endpoint API"
    );

    // TODO: Implement actual IP extraction once we determine the correct Iroh API
    // The infrastructure (database tables, repositories) is ready.

    Ok(())
}

async fn handle_message(
    database: &Database,
    paths: &GraphchanPaths,
    publisher: &Sender<NetworkEvent>,
    peer_id: Option<String>,
    payload: EventPayload,
    blobs: &FsStore,
    endpoint: &Arc<Endpoint>,
    seen_messages: &Arc<Mutex<HashSet<String>>>,
    local_peer_id: &str,
    ip_blocker: &IpBlockChecker,
) -> Result<Option<ResyncRequest>> {
    // Capture peer IP address if available
    if let Some(ref peer_id_str) = peer_id {
        if let Err(err) = capture_peer_ip(database, endpoint, peer_id_str).await {
            tracing::debug!(error = ?err, "failed to capture peer IP");
        }
    }

    match payload {
        EventPayload::ThreadAnnouncement(announcement) => {
            tracing::info!(
                thread_id = %announcement.thread_id,
                title = %announcement.title,
                post_count = announcement.post_count,
                announcer = %announcement.announcer_peer_id,
                "📢 received thread announcement (will download on-demand)"
            );

            let msg_id = format!("thread:{}:{}", announcement.thread_id, announcement.thread_hash);
            let should_rebroadcast = {
                let mut seen = seen_messages.lock().await;
                seen.insert(msg_id)
            };

            apply_thread_announcement(database, announcement.clone())?;

            // Re-broadcast only if this is the first time we've seen this version
            // CRITICAL: Change announcer_peer_id to OUR peer ID so we publish to OUR peer topic
            // This enables transitive discovery: A→B→C→... without allowing B to write to A's topic
            if should_rebroadcast {
                let publisher_clone = publisher.clone();
                let mut rebroadcast_announcement = announcement.clone();
                rebroadcast_announcement.announcer_peer_id = local_peer_id.to_string();
                tokio::spawn(async move {
                    if let Err(err) = publisher_clone.send(NetworkEvent::Broadcast(EventPayload::ThreadAnnouncement(rebroadcast_announcement))).await {
                        tracing::warn!(error = ?err, "failed to re-broadcast ThreadAnnouncement");
                    }
                });
            }

            Ok(None)
        }
        EventPayload::PostUpdate(post) => {
            tracing::info!(
                post_id = %post.id,
                thread_id = %post.thread_id,
                author = ?post.author_peer_id,
                "📝 received PostUpdate"
            );

            let msg_id = format!("post:{}", post.id);
            let should_rebroadcast = {
                let mut seen = seen_messages.lock().await;
                seen.insert(msg_id)
            };

            let result = apply_post_update(database, ip_blocker, post.clone()).await?;

            // Re-broadcast only if this is the first time we've seen this post
            // This enables transitive post propagation: A → B → C → ...
            if should_rebroadcast {
                let publisher_clone = publisher.clone();
                let post_clone = post.clone();
                tokio::spawn(async move {
                    if let Err(err) = publisher_clone.send(NetworkEvent::Broadcast(EventPayload::PostUpdate(post_clone))).await {
                        tracing::warn!(error = ?err, "failed to re-broadcast PostUpdate");
                    }
                });
            }

            Ok(result)
        }
        EventPayload::FileAvailable(announcement) => {
            tracing::debug!(
                file_id = %announcement.id,
                size_bytes = ?announcement.size_bytes,
                size_mb = announcement.size_bytes.map(|s| s / (1024 * 1024)),
                "received FileAnnouncement"
            );
            let msg_id = format!("file:{}", announcement.id);
            let should_rebroadcast = {
                let mut seen = seen_messages.lock().await;
                seen.insert(msg_id)
            };

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

            // Re-broadcast only if first time seeing this file
            if should_rebroadcast {
                let publisher_clone = publisher.clone();
                let announcement_clone = announcement.clone();
                tokio::spawn(async move {
                    if let Err(err) = publisher_clone.send(NetworkEvent::Broadcast(EventPayload::FileAvailable(announcement_clone))).await {
                        tracing::warn!(error = ?err, "failed to re-broadcast FileAvailable");
                    }
                });
            }

            Ok(None)
        }
        EventPayload::FileRequest(request) => {
            if let Some(peer_id) = peer_id {
                tracing::info!(file_id = %request.file_id, peer = %peer_id, "📤 received file request, preparing to send chunk");
                respond_with_file_chunk(database, paths, publisher, &peer_id, request)?;
            } else {
                tracing::debug!(file_id = %request.file_id, "received file request without peer_id");
            }
            Ok(None)
        }
        EventPayload::FileChunk(chunk) => {
            tracing::info!(file_id = %chunk.file_id, size = %chunk.data.len(), "📦 received file chunk");
            apply_file_chunk(database, paths, chunk)?;
            Ok(None)
        }
        EventPayload::ProfileUpdate(update) => {
            let msg_id = format!("profile:{}", update.peer_id);
            let should_rebroadcast = {
                let mut seen = seen_messages.lock().await;
                seen.insert(msg_id)
            };

            // Download avatar blob if a ticket is provided and we don't have it locally
            if let (Some(ref avatar_id), Some(ref ticket)) = (&update.avatar_file_id, &update.ticket) {
                let hash = ticket.hash();
                let has_blob = blobs.has(hash).await.unwrap_or(false);
                if !has_blob {
                    tracing::info!(
                        peer_id = %update.peer_id,
                        avatar_id = %avatar_id,
                        hash = %hash.fmt_short(),
                        "downloading avatar blob from peer"
                    );
                    let blob_store = blobs.clone();
                    let ep = endpoint.clone();
                    let peer = update.peer_id.clone();
                    let ticket = ticket.clone();
                    tokio::spawn(async move {
                        let downloader = blob_store.downloader(&ep);
                        match downloader.download(hash, Some(ticket.addr().id)).await {
                            Ok(_) => {
                                tracing::info!(peer_id = %peer, hash = %hash.fmt_short(), "avatar blob downloaded");
                            }
                            Err(err) => {
                                tracing::warn!(peer_id = %peer, error = ?err, "failed to download avatar blob");
                            }
                        }
                    });
                }
            }

            apply_profile_update(database, update.clone())?;

            // Re-broadcast profile updates only if first time seeing this update
            if should_rebroadcast {
                let publisher_clone = publisher.clone();
                let update_clone = update.clone();
                tokio::spawn(async move {
                    if let Err(err) = publisher_clone.send(NetworkEvent::Broadcast(EventPayload::ProfileUpdate(update_clone))).await {
                        tracing::warn!(error = ?err, "failed to re-broadcast ProfileUpdate");
                    }
                });
            }

            Ok(None)
        }
        EventPayload::ReactionUpdate(reaction) => {
            let msg_id = format!(
                "reaction:{}:{}:{}:{}",
                reaction.post_id,
                reaction.reactor_peer_id,
                reaction.emoji,
                reaction.is_removal
            );
            let should_rebroadcast = {
                let mut seen = seen_messages.lock().await;
                seen.insert(msg_id)
            };

            apply_reaction_update(database, reaction.clone())?;

            // Re-broadcast reaction updates only if first time seeing this update
            if should_rebroadcast {
                let publisher_clone = publisher.clone();
                let reaction_clone = reaction.clone();
                tokio::spawn(async move {
                    if let Err(err) = publisher_clone.send(NetworkEvent::Broadcast(EventPayload::ReactionUpdate(reaction_clone))).await {
                        tracing::warn!(error = ?err, "failed to re-broadcast ReactionUpdate");
                    }
                });
            }

            Ok(None)
        }

        EventPayload::DirectMessage(dm) => {
            let msg_id = format!("dm:{}", dm.message_id);
            {
                let mut seen = seen_messages.lock().await;
                if !seen.insert(msg_id) {
                    return Ok(None); // Already processed
                }
            }

            tracing::info!(
                from = %dm.from_peer_id,
                to = %dm.to_peer_id,
                message_id = %dm.message_id,
                "received DM via gossip"
            );

            // Store the DM using DmService
            let service = crate::dms::DmService::new(database.clone(), paths.clone());
            if let Err(err) = service.ingest_dm(
                &dm.from_peer_id,
                &dm.to_peer_id,
                &dm.encrypted_body,
                &dm.nonce,
                &dm.message_id,
                &dm.conversation_id,
                &dm.created_at,
            ) {
                tracing::warn!(error = ?err, "failed to ingest DM from gossip");
            }

            // Don't re-broadcast DMs - they're point-to-point
            Ok(None)
        }

        EventPayload::BlockAction(action) => {
            let msg_id = format!(
                "block:{}:{}:{}",
                action.blocker_peer_id, action.blocked_peer_id, action.is_unblock
            );
            {
                let mut seen = seen_messages.lock().await;
                if !seen.insert(msg_id) {
                    return Ok(None);
                }
            }

            tracing::info!(
                blocker = %action.blocker_peer_id,
                blocked = %action.blocked_peer_id,
                is_unblock = action.is_unblock,
                "received block action via gossip"
            );

            // Apply block action if we're subscribed to this peer's blocklist with auto_apply
            let checker = crate::blocking::BlockChecker::new(database.clone());
            if let Ok(subscriptions) = checker.list_blocklist_subscriptions() {
                for sub in &subscriptions {
                    if sub.maintainer_peer_id == action.blocker_peer_id && sub.auto_apply {
                        if action.is_unblock {
                            let _ = checker.unblock_peer(&action.blocked_peer_id);
                        } else {
                            let _ = checker.block_peer(&action.blocked_peer_id, action.reason.clone());
                        }
                        break;
                    }
                }
            }

            Ok(None)
        }
    }
}

fn apply_profile_update(database: &Database, update: ProfileUpdate) -> Result<()> {
    let service = PeerService::new(database.clone());
    service.update_profile(&update.peer_id, update.avatar_file_id, update.username, update.bio, update.agents)?;
    Ok(())
}

fn apply_reaction_update(database: &Database, reaction: ReactionUpdate) -> Result<()> {
    database.with_repositories(|repos| {
        if reaction.is_removal {
            // Remove reaction
            tracing::info!(
                post_id = %reaction.post_id,
                reactor = %reaction.reactor_peer_id,
                emoji = %reaction.emoji,
                "👎 removing reaction via gossip"
            );
            repos.reactions().remove(&reaction.post_id, &reaction.reactor_peer_id, &reaction.emoji)?;
        } else {
            // Add reaction
            tracing::info!(
                post_id = %reaction.post_id,
                reactor = %reaction.reactor_peer_id,
                emoji = %reaction.emoji,
                "👍 adding reaction via gossip"
            );
            let record = ReactionRecord {
                post_id: reaction.post_id,
                reactor_peer_id: reaction.reactor_peer_id,
                emoji: reaction.emoji,
                signature: reaction.signature,
                created_at: reaction.created_at,
            };
            repos.reactions().add(&record)?;
        }
        Ok(())
    })
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
                        visibility: existing_thread.visibility.clone(),
                        topic_secret: existing_thread.topic_secret.clone(),
                        sync_status: existing_thread.sync_status.clone(),
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
                x25519_pubkey: None,
                last_seen: None,
                avatar_file_id: None,
                trust_state: "unknown".into(),
                agents: None,
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
            visibility: "social".to_string(),
            topic_secret: None,
            sync_status: "announced".to_string(),  // Mark as announced but not yet downloaded
        };
        repos.threads().upsert(&thread_record)?;

        // Create a placeholder OP post with the preview
        // This lets the thread show up in catalog with preview text
        let op_post = PostRecord {
            id: format!("{}-preview", announcement.thread_id),
            thread_id: announcement.thread_id.clone(),
            author_peer_id: Some(announcement.creator_peer_id.clone()),
            author_friendcode: None,
            body: format!("{}...", announcement.preview),
            created_at: announcement.created_at.clone(),
            updated_at: None,
            metadata: None,
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

    // Log all files in the thread snapshot
    for post in &posts {
        for file in &post.files {
            tracing::debug!(
                thread_id = %thread.id,
                post_id = %post.id,
                file_id = %file.id,
                size_bytes = ?file.size_bytes,
                size_mb = file.size_bytes.map(|s| s / (1024 * 1024)),
                has_ticket = file.ticket.is_some(),
                "file in thread snapshot"
            );
        }
    }

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
                x25519_pubkey: peer.x25519_pubkey.clone(),
                last_seen: peer.last_seen.clone(),
                avatar_file_id: peer.avatar_file_id.clone(),
                trust_state: peer.trust_state.clone(),
                agents: peer.agents.as_ref().and_then(|a| serde_json::to_string(a).ok()),
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
                    x25519_pubkey: None,
                    last_seen: None,
                    avatar_file_id: None,
                    trust_state: "unknown".into(),
                    agents: None,
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
            visibility: thread.visibility.clone(),
            topic_secret: thread.topic_secret.clone(),
            sync_status: "downloaded".to_string(),
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
                    download_status: file.download_status.clone().or(Some("pending".to_string())),
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
            tracing::debug!(
                file_id = %file.id,
                size_bytes = ?file.size_bytes,
                size_mb = file.size_bytes.map(|s| s / (1024 * 1024)),
                has_ticket = file.ticket.is_some(),
                "checking file from thread snapshot"
            );

            let needs_fetch = file_needs_download(paths, &file)?;

            // Don't auto-download large files - let user manually trigger download
            const AUTO_DOWNLOAD_SIZE_LIMIT: i64 = 50 * 1024 * 1024; // 50MB
            let should_auto_download = if let Some(size) = file.size_bytes {
                let auto_dl = size <= AUTO_DOWNLOAD_SIZE_LIMIT;
                tracing::debug!(
                    file_id = %file.id,
                    size_mb = size / (1024 * 1024),
                    limit_mb = AUTO_DOWNLOAD_SIZE_LIMIT / (1024 * 1024),
                    should_auto_download = auto_dl,
                    "file size check result"
                );
                auto_dl
            } else {
                tracing::warn!(file_id = %file.id, "no size info, allowing auto-download");
                true // If no size info, allow auto-download
            };

            tracing::debug!(
                file_id = %file.id,
                needs_fetch,
                has_ticket = file.ticket.is_some(),
                should_auto_download,
                "download decision factors"
            );

            if needs_fetch && file.ticket.is_some() && should_auto_download {
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
            } else if needs_fetch && !should_auto_download {
                tracing::info!(
                    file_id = %file.id,
                    size_mb = file.size_bytes.unwrap_or(0) / (1024 * 1024),
                    "⏸️ file exceeds auto-download limit, marked as pending for manual download"
                );
            }
        }
    }

    Ok(())
}

/// Create a stub post for IP-blocked content to preserve graph structure
fn create_stub_post_for_blocked_ip(
    database: &Database,
    post: &PostView,
    blocked_ip: std::net::IpAddr,
) -> Result<Option<ResyncRequest>> {
    database.with_repositories(|repos| {
        // Create stub post record with placeholder body
        let stub_body = format!("[Post from IP-blocked peer: {}]", blocked_ip);

        let stub_record = PostRecord {
            id: post.id.clone(),
            thread_id: post.thread_id.clone(),
            author_peer_id: post.author_peer_id.clone(),
            author_friendcode: None, // Don't propagate friendcode from blocked user
            body: stub_body,
            created_at: post.created_at.clone(),
            updated_at: post.updated_at.clone(),
            metadata: None,
        };

        // Store stub post
        repos.posts().upsert(&stub_record)?;

        // Preserve parent relationships for DAG integrity
        repos.posts().add_relationships(&post.id, &post.parent_post_ids)?;

        tracing::info!(
            post_id = %post.id,
            thread_id = %post.thread_id,
            "✅ created stub post for IP-blocked content"
        );

        Ok(None) // No resync needed for stub posts
    })
}

async fn apply_post_update(database: &Database, ip_blocker: &IpBlockChecker, post: PostView) -> Result<Option<ResyncRequest>> {
    // Check if author's IP is blocked (using previously stored IP from peer_ips table)
    if let Some(author_id) = &post.author_peer_id {
        match ip_blocker.is_peer_blocked(author_id).await {
            Ok((true, Some(block_id), Some(ip))) => {
                // IP is blocked - create stub post instead
                tracing::info!(
                    post_id = %post.id,
                    author_id = %author_id,
                    ip = %ip,
                    block_id = block_id,
                    "🚫 blocking post from IP-blocked peer - creating stub"
                );

                // Record hit for statistics
                if let Err(err) = ip_blocker.record_hit(block_id).await {
                    tracing::warn!(error = ?err, "failed to record IP block hit");
                }

                // Create stub post that preserves graph structure
                return create_stub_post_for_blocked_ip(database, &post, ip);
            }
            Ok((true, None, Some(ip))) => {
                // Blocked but no block_id (shouldn't happen but handle gracefully)
                tracing::warn!(
                    post_id = %post.id,
                    author_id = %author_id,
                    ip = %ip,
                    "IP blocked but no block_id found - creating stub anyway"
                );
                return create_stub_post_for_blocked_ip(database, &post, ip);
            }
            Ok((true, _, None)) => {
                // Blocked but couldn't determine IP (shouldn't happen)
                tracing::warn!(
                    post_id = %post.id,
                    author_id = %author_id,
                    "peer marked as blocked but IP unknown - allowing post"
                );
            }
            Ok((false, _, _)) => {
                // Not blocked - proceed normally
            }
            Err(err) => {
                tracing::warn!(
                    error = ?err,
                    post_id = %post.id,
                    "failed to check IP block status - allowing post"
                );
            }
        }
    }

    database.with_repositories(|repos| {
        if repos.threads().get(&post.thread_id)?.is_none() {
            tracing::warn!(
                thread_id = %post.thread_id,
                post_id = %post.id,
                "⚠️ skipping PostUpdate - thread unknown (may need to download thread first)"
            );
            return Ok(None);
        }

        // Check thread hash for synchronization
        let mut resync_request = None;
        if let Some(remote_hash) = &post.thread_hash {
            // Get current local posts and calculate local hash
            let posts = repos.posts().list_for_thread(&post.thread_id)?;
            let local_posts: Vec<PostView> = posts.iter().map(|p| {
                let parents = repos.posts().parents_of(&p.id).unwrap_or_default();
                let files = repos.files().list_for_post(&p.id).unwrap_or_default();
                let file_views = files.into_iter()
                    .map(crate::files::FileView::from_record)
                    .collect();
                // Parse metadata JSON if present
                let metadata = p.metadata.as_ref().and_then(|json_str| {
                    serde_json::from_str::<crate::threading::PostMetadata>(json_str).ok()
                });
                PostView {
                    id: p.id.clone(),
                    thread_id: p.thread_id.clone(),
                    author_peer_id: p.author_peer_id.clone(),
                    author_friendcode: p.author_friendcode.clone(),
                    body: p.body.clone(),
                    created_at: p.created_at.clone(),
                    updated_at: p.updated_at.clone(),
                    parent_post_ids: parents,
                    files: file_views,
                    thread_hash: None,
                    metadata,
                }
            }).collect();

            let local_hash = crate::threading::calculate_thread_hash(&local_posts);

            if &local_hash != remote_hash {
                tracing::warn!(
                    thread_id = %post.thread_id,
                    post_id = %post.id,
                    local_hash = %local_hash,
                    remote_hash = %remote_hash,
                    "🔄 thread hash mismatch detected - will trigger auto-resync"
                );

                // Check if we have a download ticket for this thread
                let ticket_result = repos.conn().query_row(
                    "SELECT ticket FROM thread_tickets WHERE thread_id = ?1",
                    rusqlite::params![post.thread_id],
                    |row| row.get::<_, String>(0),
                )
                .optional()
                .context("failed to query thread_tickets");

                if let Ok(Some(ticket_str)) = ticket_result {
                    match ticket_str.parse::<BlobTicket>() {
                        Ok(ticket) => {
                            resync_request = Some(ResyncRequest {
                                thread_id: post.thread_id.clone(),
                                ticket,
                            });
                        }
                        Err(e) => {
                            tracing::warn!(
                                error = ?e,
                                thread_id = %post.thread_id,
                                "invalid blob ticket, cannot auto-resync"
                            );
                        }
                    }
                } else {
                    tracing::warn!(
                        thread_id = %post.thread_id,
                        "no download ticket available, cannot auto-resync (need ThreadAnnouncement)"
                    );
                }
            } else {
                tracing::debug!(
                    thread_id = %post.thread_id,
                    hash = %local_hash,
                    "thread hash matches - in sync"
                );
            }
        }

        // Ensure the author peer exists - create/update peer with friend code from post
        // Extract IPs from embedded friendcode for IP blocking
        let mut extracted_ips = Vec::new();

        if let Some(author_id) = &post.author_peer_id {
            let peers_repo = repos.peers();
            let existing_peer = peers_repo.get(author_id)?;

            // Extract friend code info and IPs from post if available
            let (friend_iroh_peer_id, friend_gpg_fingerprint, _friend_x25519, _addresses) =
                if let Some(friendcode_str) = &post.author_friendcode {
                    // Try to decode as full legacy friendcode (v1/v2 with multiaddrs)
                    match crate::identity::decode_friendcode_auto(friendcode_str) {
                        Ok(payload) => {
                            // Extract and store IP addresses from multiaddrs
                            let ips = crate::peers::extract_ips_from_multiaddrs(&payload.addresses);
                            if !ips.is_empty() {
                                let timestamp = chrono::Utc::now().timestamp();
                                for ip in &ips {
                                    if let Err(err) = repos.peer_ips().update(author_id, &ip.to_string(), timestamp) {
                                        tracing::debug!(error = ?err, peer_id = %author_id, ip = %ip, "failed to store peer IP from post");
                                    }
                                }
                                tracing::debug!(
                                    peer_id = %author_id,
                                    ip_count = ips.len(),
                                    "extracted IP addresses from post friendcode"
                                );
                                // Save IPs for blocking check
                                extracted_ips = ips;
                            }
                            (Some(payload.peer_id), Some(payload.gpg_fingerprint), payload.x25519_pubkey, payload.addresses)
                        }
                        Err(err) => {
                            tracing::debug!(error = ?err, "failed to decode author_friendcode from post");
                            (None, None, None, vec![])
                        }
                    }
                } else {
                    (None, None, None, vec![])
                };

            if let Some(mut peer) = existing_peer {
                // Update existing peer with friend code info if we have it and peer doesn't
                let mut needs_update = false;
                if friend_iroh_peer_id.is_some() && peer.iroh_peer_id.is_none() {
                    peer.iroh_peer_id = friend_iroh_peer_id.clone();
                    peer.friendcode = post.author_friendcode.clone();
                    needs_update = true;
                }
                if needs_update {
                    tracing::info!(peer_id = %author_id, "updating peer with friend code from post");
                    peers_repo.upsert(&peer)?;
                }
            } else {
                // Create new peer record
                tracing::info!(peer_id = %author_id, "creating peer for unknown post author");
                let stub_peer = crate::database::models::PeerRecord {
                    id: author_id.clone(),
                    alias: None,
                    username: Some(format!("Unknown ({})", &author_id[..8])),
                    bio: None,
                    friendcode: post.author_friendcode.clone(),
                    iroh_peer_id: friend_iroh_peer_id,
                    gpg_fingerprint: friend_gpg_fingerprint.or_else(|| Some(author_id.clone())),
                    x25519_pubkey: None,
                    last_seen: None,
                    avatar_file_id: None,
                    trust_state: "unknown".into(),
                    agents: None,
                };
                peers_repo.upsert(&stub_peer)?;
            }
        }

        let posts_repo = repos.posts();
        upsert_post(&posts_repo, &post)?;

        tracing::info!(
            post_id = %post.id,
            thread_id = %post.thread_id,
            "✅ applied PostUpdate successfully"
        );

        Ok(resync_request)
    })
}

fn upsert_post<R>(repo: &R, post: &PostView) -> Result<()>
where
    R: PostRepository,
{
    // Serialize metadata to JSON if present
    let metadata_json = post.metadata.as_ref().and_then(|meta| {
        serde_json::to_string(meta).ok()
    });

    let record = PostRecord {
        id: post.id.clone(),
        thread_id: post.thread_id.clone(),
        author_peer_id: post.author_peer_id.clone(),
        author_friendcode: post.author_friendcode.clone(),
        body: post.body.clone(),
        created_at: post.created_at.clone(),
        updated_at: post.updated_at.clone(),
        metadata: metadata_json,
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
        download_status: Some("pending".to_string()),
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
    tracing::debug!(
        file_id = %announcement.id,
        needs_fetch = %needs_fetch,
        size_bytes = ?record.size_bytes,
        "checked if download needed"
    );

    // Don't auto-download large files - let user manually trigger download
    const AUTO_DOWNLOAD_SIZE_LIMIT: i64 = 50 * 1024 * 1024; // 50MB
    if needs_fetch {
        if let Some(size) = record.size_bytes {
            tracing::debug!(
                file_id = %announcement.id,
                size_bytes = size,
                size_mb = size / (1024 * 1024),
                limit_mb = AUTO_DOWNLOAD_SIZE_LIMIT / (1024 * 1024),
                "checking if file exceeds auto-download limit"
            );
            if size > AUTO_DOWNLOAD_SIZE_LIMIT {
                tracing::info!(
                    file_id = %announcement.id,
                    size_mb = size / (1024 * 1024),
                    "⏸️ file exceeds auto-download limit ({}MB), marked as pending for manual download",
                    AUTO_DOWNLOAD_SIZE_LIMIT / (1024 * 1024)
                );
                ensure_download_directory(paths)?;
                return Ok(false); // Don't auto-download, user must trigger manually
            } else {
                tracing::debug!(
                    file_id = %announcement.id,
                    size_mb = size / (1024 * 1024),
                    "file is small enough for auto-download"
                );
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
        }).await;

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

                // Set status to 'failed'
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
                }).await;

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
            record.download_status = Some("available".to_string());
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
                    visibility: "social".to_string(),
                    topic_secret: None,
                    sync_status: "downloaded".to_string(),
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
                "test-peer-id".to_string(),
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
