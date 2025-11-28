use crate::config::{GraphchanPaths, NetworkConfig};
mod events;
pub mod ingest;

use crate::database::Database;
use crate::identity::{load_iroh_secret, FriendCodePayload};
use crate::threading::{PostView, ThreadDetails};
use anyhow::{Context, Result};
use events::{EventPayload, NetworkEvent};
use iroh::endpoint::{Endpoint, RelayMode};
use iroh::protocol::Router;
use iroh_base::{EndpointAddr, PublicKey, RelayUrl};
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::{ticket::BlobTicket, BlobFormat, BlobsProtocol, Hash, ALPN as BLOBS_ALPN};
use iroh_gossip::api::GossipTopic;
use iroh_gossip::net::Gossip;
use iroh_relay::RelayMap;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

const GRAPHCHAN_ALPN: &[u8] = b"graphchan/0";
const GOSSIP_BUFFER: usize = 128;

pub use events::FileAnnouncement;
pub use events::ProfileUpdate;

type TopicId = iroh_gossip::proto::TopicId;

#[derive(Clone)]
pub struct NetworkHandle {
    endpoint: Arc<Endpoint>,
    gossip: Gossip,
    publisher: mpsc::Sender<events::NetworkEvent>,
    inbound_tx: mpsc::Sender<events::InboundGossip>,
    _event_worker: Arc<JoinHandle<()>>,
    _ingest_worker: Arc<JoinHandle<()>>,
    _router: Arc<Router>,
    topics: Arc<RwLock<HashMap<String, GossipTopic>>>,
    blobs: FsStore,
}

impl NetworkHandle {
    pub async fn start(
        paths: &GraphchanPaths,
        config: &NetworkConfig,
        blob_store: FsStore,
        database: Database,
    ) -> Result<Self> {
        let secret = load_iroh_secret(paths)?;
        let mut builder = Endpoint::builder().secret_key(secret);

        if let Some(relay_url) = &config.relay_url {
            let relay_url: RelayUrl = relay_url
                .parse()
                .with_context(|| format!("invalid GRAPHCHAN_RELAY_URL value: {relay_url}"))?;
            let relay_map: RelayMap = relay_url.into();
            builder = builder.relay_mode(RelayMode::Custom(relay_map));
        }

        let endpoint = builder
            .bind()
            .await
            .context("failed to bind iroh endpoint")?;
        let endpoint = Arc::new(endpoint);

        let gossip = Gossip::builder()
            .alpn(GRAPHCHAN_ALPN)
            .spawn(endpoint.as_ref().clone());

        let (tx, rx) = mpsc::channel(GOSSIP_BUFFER);
        let event_worker_gossip = gossip.clone();
        let event_worker_topics = Arc::new(RwLock::new(HashMap::new()));

        // No longer using global topic - peer-based gossip only
        // Each peer will subscribe to their friends' peer-{id} topics

        let event_worker_topics_clone = event_worker_topics.clone();
        let event_worker = tokio::spawn(async move {
            events::run_event_loop(event_worker_gossip, event_worker_topics_clone, rx).await;
        });

        let (inbound_tx, inbound_rx) = mpsc::channel(GOSSIP_BUFFER);
        let ingest_publisher = tx.clone();
        let ingest_database = database.clone();
        let ingest_paths = paths.clone();
        let ingest_store = blob_store.clone();
        let ingest_endpoint = endpoint.clone();
        let ingest_worker = tokio::spawn(async move {
            ingest::run_ingest_loop(
                ingest_database,
                ingest_paths,
                ingest_publisher,
                inbound_rx,
                ingest_store,
                ingest_endpoint,
            )
            .await;
        });

        // No longer need global receiver loop - each subscribe_to_peer/subscribe_to_thread
        // spawns its own receiver task

        let blob_protocol = BlobsProtocol::new(&blob_store, None);
        let router = Router::builder(endpoint.as_ref().clone())
            .accept(GRAPHCHAN_ALPN, gossip.clone())
            .accept(BLOBS_ALPN, blob_protocol)
            .spawn();
        let router = Arc::new(router);

        let handle = Self {
            endpoint,
            gossip,
            publisher: tx,
            inbound_tx,
            _event_worker: Arc::new(event_worker),
            _ingest_worker: Arc::new(ingest_worker),
            _router: router.clone(),
            topics: event_worker_topics,
            blobs: blob_store.clone(),
        };
        tracing::info!(peer_id = %handle.peer_id(), "iroh endpoint started");

        // Get our own identity to subscribe to our own peer topic
        if let Ok(Some((fingerprint, _, _))) = database.get_identity() {
            tracing::info!(peer_id = %fingerprint, "subscribing to own peer topic");
            handle.subscribe_to_peer(&fingerprint, None).await?;
        }

        // Subscribe to all existing friends' peer topics
        let peer_service = crate::peers::PeerService::new(database.clone());
        if let Ok(peers) = peer_service.list_peers() {
            for peer in peers {
                // Don't re-subscribe to ourselves
                if Some(&peer.id) != database.get_identity().ok().flatten().as_ref().map(|(f, _, _)| f) {
                    tracing::info!(peer_id = %peer.id, "subscribing to friend's peer topic on startup");
                    // On startup we don't have an active connection, so no bootstrap peer
                    // The gossip network will find neighbors through DHT discovery
                    handle.subscribe_to_peer(&peer.id, None).await?;
                }
            }
        }

        Ok(handle)
    }

    pub fn peer_id(&self) -> String {
        self.endpoint.id().to_string()
    }

    pub fn current_addr(&self) -> EndpointAddr {
        self.endpoint.addr()
    }

    pub fn make_blob_ticket(&self, blob_hex: &str) -> Option<BlobTicket> {
        match blob_hex.parse::<Hash>() {
            Ok(hash) => Some(BlobTicket::new(self.current_addr(), hash, BlobFormat::Raw)),
            Err(err) => {
                tracing::warn!(error = %err, "failed to parse blob id for ticket");
                None
            }
        }
    }

    pub fn endpoint(&self) -> Arc<Endpoint> {
        self.endpoint.clone()
    }

    /// Broadcasts a thread announcement (metadata + blob ticket).
    /// Always stores full thread as blob, sends lightweight announcement via gossip.
    pub async fn publish_thread_announcement(&self, snapshot: ThreadDetails, local_peer_id: &str) -> Result<()> {
        // Store complete thread as blob
        let json_bytes = serde_json::to_vec(&snapshot)?;
        let size = json_bytes.len();

        let outcome = self.blobs
            .add_bytes(json_bytes)
            .await
            .context("failed to add thread to blob store")?;

        let hash = outcome.hash;

        // Create ticket for downloading
        let addr = self.current_addr();
        let ticket = iroh_blobs::ticket::BlobTicket::new(addr, hash, iroh_blobs::BlobFormat::Raw);

        // Extract preview (first 140 chars of OP body)
        let preview = snapshot.posts.first()
            .and_then(|p| Some(p.body.chars().take(140).collect::<String>()))
            .unwrap_or_default();

        // Check if thread has any images
        let has_images = snapshot.posts.iter()
            .any(|p| !p.files.is_empty());

        // Get last activity timestamp
        let last_activity = snapshot.posts.iter()
            .map(|p| p.created_at.as_str())
            .max()
            .unwrap_or(&snapshot.thread.created_at)
            .to_string();

        // Calculate thread hash from all posts
        let thread_hash = crate::threading::calculate_thread_hash(&snapshot.posts);

        // Create lightweight announcement
        let announcement = events::ThreadAnnouncement {
            thread_id: snapshot.thread.id.clone(),
            creator_peer_id: snapshot.thread.creator_peer_id.clone().unwrap_or_else(|| local_peer_id.to_string()),
            announcer_peer_id: local_peer_id.to_string(),
            title: snapshot.thread.title.clone(),
            preview,
            ticket,
            post_count: snapshot.posts.len(),
            has_images,
            created_at: snapshot.thread.created_at.clone(),
            last_activity,
            thread_hash,
        };

        tracing::info!(
            thread_id = %snapshot.thread.id,
            post_count = snapshot.posts.len(),
            size_kb = format!("{:.2}", size as f64 / 1024.0),
            "📢 broadcasting thread announcement (full thread stored as blob)"
        );

        let event = NetworkEvent::Broadcast(EventPayload::ThreadAnnouncement(announcement));
        self.publisher.send(event).await.ok();

        // Subscribe to this thread's topic to receive future updates
        self.subscribe_to_thread(&snapshot.thread.id).await?;

        Ok(())
    }

    /// Broadcasts a single post update to connected peers.
    pub async fn publish_post_update(&self, post: PostView) -> Result<()> {
        let event = NetworkEvent::Broadcast(EventPayload::PostUpdate(post));
        self.publisher.send(event).await.ok();
        Ok(())
    }

    /// Broadcasts that an attachment blob is available for download.
    pub async fn publish_file_available(&self, announcement: FileAnnouncement) -> Result<()> {
        let event = NetworkEvent::Broadcast(EventPayload::FileAvailable(announcement));
        self.publisher.send(event).await.ok();
        Ok(())
    }

    pub async fn publish_profile_update(&self, update: ProfileUpdate) -> Result<()> {
        let event = NetworkEvent::Broadcast(EventPayload::ProfileUpdate(update));
        self.publisher.send(event).await.ok();
        Ok(())
    }

    /// Requests a file blob from a specific peer.
    pub async fn request_file(&self, peer_id: &str, file_id: &str) -> Result<()> {
        let event = NetworkEvent::Direct {
            peer_id: peer_id.to_string(),
            payload: EventPayload::FileRequest(events::FileRequest {
                file_id: file_id.to_string(),
            }),
        };
        self.publisher.send(event).await.ok();
        Ok(())
    }

    /// Subscribe to a peer's topic to receive their announcements.
    /// Call this when adding a friend or on startup for existing friends.
    ///
    /// If bootstrap_peer is provided, it will be used to help discover gossip neighbors.
    pub async fn subscribe_to_peer(&self, peer_id: &str, bootstrap_peer: Option<iroh::PublicKey>) -> Result<()> {
        let topic_name = format!("peer-{}", peer_id);
        let topic_id = TopicId::from_bytes(*blake3::hash(topic_name.as_bytes()).as_bytes());

        let bootstrap_peers = bootstrap_peer.map(|p| vec![p]).unwrap_or_default();

        tracing::info!(
            peer_id = %peer_id,
            topic = %topic_name,
            has_bootstrap = bootstrap_peer.is_some(),
            "subscribing to peer topic"
        );

        // Subscribe to receive messages from this peer's topic
        let mut receiver = self.gossip.subscribe(topic_id, bootstrap_peers).await?;
        let inbound_tx = self.inbound_tx.clone();

        // Spawn a task to forward messages from this peer's topic to the ingest loop
        tokio::spawn(async move {
            use futures_util::StreamExt;
            while let Some(event_result) = receiver.next().await {
                match event_result {
                    Ok(iroh_gossip::api::Event::Received(message)) => {
                        match serde_json::from_slice::<events::EventEnvelope>(&message.content) {
                            Ok(envelope) => {
                                let gossip = events::InboundGossip {
                                    peer_id: Some(message.delivered_from.to_string()),
                                    payload: envelope.payload,
                                };
                                if inbound_tx.send(gossip).await.is_err() {
                                    tracing::warn!(topic = %topic_name, "inbound channel closed, stopping peer topic receiver");
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::warn!(error = ?err, topic = %topic_name, "failed to deserialize message on peer topic");
                            }
                        }
                    }
                    Ok(iroh_gossip::api::Event::NeighborUp(neighbor_id)) => {
                        tracing::info!(peer = %neighbor_id.fmt_short(), topic = %topic_name, "neighbor up on peer topic");
                    }
                    Ok(iroh_gossip::api::Event::NeighborDown(neighbor_id)) => {
                        tracing::info!(peer = %neighbor_id.fmt_short(), topic = %topic_name, "neighbor down on peer topic");
                    }
                    Ok(iroh_gossip::api::Event::Lagged) => {
                        tracing::warn!(topic = %topic_name, "gossip receiver lagged on peer topic");
                    }
                    Err(err) => {
                        tracing::warn!(error = ?err, topic = %topic_name, "error in peer topic receiver");
                    }
                }
            }
            tracing::info!(topic = %topic_name, "peer topic receiver loop ended");
        });

        Ok(())
    }

    /// Subscribe to a thread-specific topic to receive PostUpdates and FileAnnouncements.
    /// Call this after downloading a thread to start receiving updates.
    pub async fn subscribe_to_thread(&self, thread_id: &str) -> Result<()> {
        let topic_name = format!("thread-{}", thread_id);
        let topic_id = TopicId::from_bytes(*blake3::hash(topic_name.as_bytes()).as_bytes());

        tracing::info!(thread_id = %thread_id, topic = %topic_name, "subscribing to thread topic for receiving updates");

        // Subscribe to receive messages on this thread's topic
        let mut receiver = self.gossip.subscribe(topic_id, vec![]).await?;
        let inbound_tx = self.inbound_tx.clone();

        // Spawn a task to forward messages from this topic to the ingest loop
        tokio::spawn(async move {
            use futures_util::StreamExt;
            while let Some(event_result) = receiver.next().await {
                match event_result {
                    Ok(iroh_gossip::api::Event::Received(message)) => {
                        match serde_json::from_slice::<events::EventEnvelope>(&message.content) {
                            Ok(envelope) => {
                                let gossip = events::InboundGossip {
                                    peer_id: Some(message.delivered_from.to_string()),
                                    payload: envelope.payload,
                                };
                                if inbound_tx.send(gossip).await.is_err() {
                                    tracing::warn!(topic = %topic_name, "inbound channel closed, stopping thread topic receiver");
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::warn!(error = ?err, topic = %topic_name, "failed to deserialize message on thread topic");
                            }
                        }
                    }
                    Ok(iroh_gossip::api::Event::NeighborUp(peer_id)) => {
                        tracing::debug!(peer = %peer_id.fmt_short(), topic = %topic_name, "neighbor up on thread topic");
                    }
                    Ok(iroh_gossip::api::Event::NeighborDown(peer_id)) => {
                        tracing::debug!(peer = %peer_id.fmt_short(), topic = %topic_name, "neighbor down on thread topic");
                    }
                    Ok(iroh_gossip::api::Event::Lagged) => {
                        tracing::warn!(topic = %topic_name, "gossip receiver lagged, some messages may have been dropped");
                    }
                    Err(err) => {
                        tracing::warn!(error = ?err, topic = %topic_name, "error in thread topic receiver");
                    }
                }
            }
            tracing::info!(topic = %topic_name, "thread topic receiver loop ended");
        });

        Ok(())
    }

    /// Returns the list of currently connected peer IDs.
    pub async fn connected_peer_ids(&self) -> Vec<String> {
        // For iroh-gossip, we can't easily query all neighbors from all topics
        // without consuming the receivers. Return empty for now.
        // In a real implementation, you'd track this separately or use a different approach.
        Vec::new()
    }

    pub async fn connect_friendcode(&self, payload: &FriendCodePayload) -> Result<iroh::PublicKey> {
        let addr = build_endpoint_addr(payload)?;
        let peer_id = addr.id;

        tracing::info!(peer = %peer_id.fmt_short(), "attempting to connect to peer");
        self.endpoint.connect(addr.clone(), GRAPHCHAN_ALPN).await.context("failed to connect to peer")?;
        tracing::info!(peer = %peer_id.fmt_short(), "✅ endpoint connected!");

        Ok(peer_id)
    }

    pub async fn shutdown(&self) {
        let _ = self._router.shutdown().await;
        self.endpoint.close().await;
    }
}

fn build_endpoint_addr(payload: &FriendCodePayload) -> Result<EndpointAddr> {
    let peer_id = PublicKey::from_str(&payload.peer_id)
        .with_context(|| format!("invalid peer id in friendcode: {}", payload.peer_id))?;
    let mut addr = EndpointAddr::new(peer_id);

    for entry in &payload.addresses {
        if let Ok(relay) = entry.parse::<RelayUrl>() {
            addr = addr.with_relay_url(relay);
            continue;
        }

        if let Ok(sock) = entry.parse::<SocketAddr>() {
            addr = addr.with_ip_addr(sock);
            continue;
        }

        tracing::debug!(address = entry, "unrecognized friendcode address, ignoring");
    }

    Ok(addr)
}
