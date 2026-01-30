use crate::config::{GraphchanPaths, NetworkConfig};
mod events;
pub mod ingest;
pub mod topics;

use crate::database::Database;
use crate::database::repositories::{ThreadRepository, PostRepository, PeerRepository};
use crate::identity::{load_iroh_secret, FriendCodePayload};
use crate::threading::{PostView, ThreadDetails};
use anyhow::{Context, Result};
use events::{EventPayload, NetworkEvent};
use iroh::endpoint::{Endpoint, RelayMode};
use iroh::discovery::pkarr::dht::DhtDiscovery;
use iroh::discovery::mdns::MdnsDiscovery;
use iroh::protocol::Router;
use iroh_base::{EndpointAddr, PublicKey, RelayUrl};
use iroh_blobs::store::fs::FsStore;
use iroh_blobs::{ticket::BlobTicket, BlobFormat, BlobsProtocol, Hash, ALPN as BLOBS_ALPN};
use iroh_gossip::api::GossipTopic;
use iroh_gossip::net::Gossip;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

// DHT-based topic discovery
use distributed_topic_tracker::{RecordPublisher, TopicId as DhtTopicId};
use mainline::Dht;
use std::sync::atomic::{AtomicBool, Ordering};

const GRAPHCHAN_ALPN: &[u8] = b"graphchan/0";
const GOSSIP_BUFFER: usize = 128;

pub use events::FileAnnouncement;
pub use events::ProfileUpdate;
pub use events::ReactionUpdate;

type TopicId = iroh_gossip::proto::TopicId;

/// Status of DHT connectivity
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DhtStatus {
    /// DHT check hasn't completed yet
    Checking,
    /// DHT is connected and bootstrapped
    Connected,
    /// DHT bootstrap failed (network/NAT issue)
    Unreachable,
}

/// Wrapper for DHT topic sender to enable Clone
#[derive(Clone)]
pub struct DhtTopicSender(Arc<tokio::sync::Mutex<Option<distributed_topic_tracker::GossipSender>>>);

impl DhtTopicSender {
    fn new(sender: distributed_topic_tracker::GossipSender) -> Self {
        Self(Arc::new(tokio::sync::Mutex::new(Some(sender))))
    }

    pub async fn broadcast(&self, data: bytes::Bytes) -> anyhow::Result<()> {
        let mut guard = self.0.lock().await;
        if let Some(sender) = guard.as_mut() {
            sender.broadcast(data.to_vec()).await?;
        }
        Ok(())
    }
}

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
    /// DHT topic senders for broadcasting to DHT-discovered peers
    dht_senders: Arc<RwLock<HashMap<String, DhtTopicSender>>>,
    blobs: FsStore,
    database: Database,
    /// Whether DHT is reachable (true = connected, false = unreachable, None = still checking)
    dht_connected: Arc<AtomicBool>,
    dht_checked: Arc<AtomicBool>,
}

impl NetworkHandle {
    pub async fn start(
        paths: &GraphchanPaths,
        config: &NetworkConfig,
        blob_store: FsStore,
        database: Database,
        local_peer_id: String,
    ) -> Result<Self> {
        let secret = load_iroh_secret(paths)?;
        let endpoint_id = secret.public();

        // Note: DHT signing keys are derived per-topic in subscribe_to_topic()
        // so all peers on the same topic share the same key for verification

        // Determine relay mode
        let relay_mode = if let Some(relay_url) = &config.relay_url {
            let relay_url: RelayUrl = relay_url
                .parse()
                .with_context(|| format!("invalid GRAPHCHAN_RELAY_URL value: {relay_url}"))?;
            tracing::info!(relay = %relay_url, "using custom relay server");
            RelayMode::Custom(relay_url.into())
        } else {
            // Use default public relays (n0's servers)
            tracing::info!("using default public relays (n0/iroh team)");
            RelayMode::Default
        };

        // Create endpoint builder with relay mode
        let mut builder = Endpoint::empty_builder(relay_mode).secret_key(secret);

        // Conditionally add DHT discovery if enabled
        if config.enable_dht {
            let dht_discovery = DhtDiscovery::builder()
                .build()
                .context("Failed to create DHT discovery")?;
            builder = builder.discovery(dht_discovery);
            tracing::info!("DHT discovery enabled (BitTorrent mainline)");
        } else {
            tracing::info!("DHT discovery disabled via config");
        }

        // Conditionally add mDNS discovery if enabled
        if config.enable_mdns {
            let mdns_discovery = MdnsDiscovery::builder()
                .build(endpoint_id)
                .context("Failed to create mDNS discovery")?;
            builder = builder.discovery(mdns_discovery);
            tracing::info!("mDNS discovery enabled (local network)");
        } else {
            tracing::info!("mDNS discovery disabled via config");
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
        let event_worker_dht_senders: Arc<RwLock<HashMap<String, DhtTopicSender>>> = Arc::new(RwLock::new(HashMap::new()));

        // No longer using global topic - peer-based gossip only
        // Each peer will subscribe to their friends' peer-{id} topics

        let event_worker_topics_clone = event_worker_topics.clone();
        let event_worker_dht_senders_clone = event_worker_dht_senders.clone();
        let event_worker = tokio::spawn(async move {
            events::run_event_loop(event_worker_gossip, event_worker_topics_clone, event_worker_dht_senders_clone, rx).await;
        });

        let (inbound_tx, inbound_rx) = mpsc::channel(GOSSIP_BUFFER);
        let ingest_publisher = tx.clone();
        let ingest_database = database.clone();
        let ingest_paths = paths.clone();
        let ingest_store = blob_store.clone();
        let ingest_endpoint = endpoint.clone();
        let ingest_local_peer_id = local_peer_id.clone();

        // Create IP blocker and load cache
        let ip_blocker = crate::blocking::IpBlockChecker::new(database.clone());
        if let Err(err) = ip_blocker.load_cache().await {
            tracing::warn!(error = ?err, "failed to load IP block cache");
        }

        let ingest_worker = tokio::spawn(async move {
            ingest::run_ingest_loop(
                ingest_database,
                ingest_paths,
                ingest_publisher,
                inbound_rx,
                ingest_store,
                ingest_endpoint,
                ingest_local_peer_id,
                ip_blocker,
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

        let dht_connected = Arc::new(AtomicBool::new(false));
        let dht_checked = Arc::new(AtomicBool::new(false));

        let handle = Self {
            endpoint,
            gossip,
            publisher: tx,
            inbound_tx,
            _event_worker: Arc::new(event_worker),
            _ingest_worker: Arc::new(ingest_worker),
            _router: router.clone(),
            topics: event_worker_topics,
            dht_senders: event_worker_dht_senders,
            blobs: blob_store.clone(),
            database: database.clone(),
            dht_connected: dht_connected.clone(),
            dht_checked: dht_checked.clone(),
        };
        tracing::info!(peer_id = %handle.peer_id(), "iroh endpoint started");

        // Spawn DHT connectivity check in background
        if config.enable_dht {
            tokio::spawn(async move {
                tracing::info!("🔍 checking DHT connectivity...");
                match check_dht_connectivity().await {
                    Ok(true) => {
                        dht_connected.store(true, Ordering::SeqCst);
                        dht_checked.store(true, Ordering::SeqCst);
                        tracing::info!("✅ DHT is REACHABLE - topic discovery will work");
                    }
                    Ok(false) => {
                        dht_connected.store(false, Ordering::SeqCst);
                        dht_checked.store(true, Ordering::SeqCst);
                        tracing::warn!("❌ DHT is UNREACHABLE - topic discovery will NOT work (check NAT/firewall)");
                    }
                    Err(err) => {
                        dht_connected.store(false, Ordering::SeqCst);
                        dht_checked.store(true, Ordering::SeqCst);
                        tracing::error!(error = ?err, "❌ DHT connectivity check failed");
                    }
                }
            });
        } else {
            dht_checked.store(true, Ordering::SeqCst);
            tracing::info!("DHT disabled, skipping connectivity check");
        }

        // Get our own identity to subscribe to our own peer topic
        if let Ok(Some((fingerprint, _, _))) = database.get_identity() {
            tracing::info!(peer_id = %fingerprint, "subscribing to own peer topic");
            handle.subscribe_to_peer(&fingerprint, None).await?;
        }

        // Subscribe to global discovery topic (unless opted out)
        let opt_out_global = database
            .get_setting("opt_out_global_discovery")
            .unwrap_or(Some("false".to_string()))
            .unwrap_or_else(|| "false".to_string())
            == "true";

        if !opt_out_global {
            tracing::info!("subscribing to global discovery topic");
            handle.subscribe_to_global().await?;
        } else {
            tracing::info!("global discovery opted out via settings");
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

        // Subscribe to all existing threads to receive updates
        let thread_service = crate::threading::ThreadService::new(database.clone());
        if let Ok(threads) = thread_service.list_threads(10000) {
            for thread in threads {
                tracing::info!(thread_id = %thread.id, "subscribing to thread topic on startup");
                handle.subscribe_to_thread(&thread.id).await?;
            }
        }

        // Subscribe to all user topics for public discovery
        use crate::database::repositories::TopicRepository;
        if let Ok(topics) = database.with_repositories(|repos| repos.topics().list_subscribed()) {
            for topic_id in topics {
                tracing::info!(topic = %topic_id, "subscribing to user topic on startup");
                handle.subscribe_to_topic(&topic_id).await?;
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

    /// Get current network addresses (relay URLs and IP addresses) for friend code generation
    pub fn get_addresses(&self) -> Vec<String> {
        let addr = self.endpoint.addr();
        let mut addresses = Vec::new();

        // Add relay URLs first (most important for NAT traversal)
        for relay in addr.relay_urls() {
            addresses.push(relay.to_string());
        }

        // Add direct IP addresses
        for ip in addr.ip_addrs() {
            addresses.push(ip.to_string());
        }

        addresses
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

        // Get topics for this thread from database
        use crate::database::repositories::TopicRepository;
        let topics = self.database.with_repositories(|repos| {
            repos.topics().list_thread_topics(&snapshot.thread.id)
        }).unwrap_or_default();

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
            visibility: snapshot.thread.visibility.clone(),
            topics,
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

    pub async fn publish_reaction_update(&self, update: ReactionUpdate) -> Result<()> {
        let event = NetworkEvent::Broadcast(EventPayload::ReactionUpdate(update));
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

    /// Subscribe to the global discovery topic to receive announcements from all nodes.
    /// This is called automatically on startup unless the user has opted out.
    pub async fn subscribe_to_global(&self) -> Result<()> {
        use crate::network::topics::{GLOBAL_TOPIC_NAME, derive_global_topic};

        let topic_name = GLOBAL_TOPIC_NAME.to_string();
        let topic_id = TopicId::from_bytes(derive_global_topic());

        tracing::info!(topic = %topic_name, "subscribing to global discovery topic");

        // Create two subscriptions to this topic:
        // 1. For receiving (consumed by the spawned task below)
        // 2. For broadcasting (stored in topics map for the event worker)
        let receiver_topic = self.gossip.subscribe(topic_id, vec![]).await?;
        let broadcaster_topic = self.gossip.subscribe(topic_id, vec![]).await?;

        // Store broadcaster in topics map
        {
            let mut topics_guard = self.topics.write().await;
            topics_guard.insert(topic_name.clone(), broadcaster_topic);
        }

        let mut receiver = receiver_topic;
        let inbound_tx = self.inbound_tx.clone();

        // Spawn a task to forward messages from the global topic to the ingest loop
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
                                    tracing::warn!(topic = %topic_name, "inbound channel closed, stopping global topic receiver");
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::warn!(error = ?err, topic = %topic_name, "failed to deserialize message on global topic");
                            }
                        }
                    }
                    Ok(iroh_gossip::api::Event::NeighborUp(neighbor_id)) => {
                        tracing::debug!(peer = %neighbor_id.fmt_short(), topic = %topic_name, "neighbor up on global topic");
                    }
                    Ok(iroh_gossip::api::Event::NeighborDown(neighbor_id)) => {
                        tracing::debug!(peer = %neighbor_id.fmt_short(), topic = %topic_name, "neighbor down on global topic");
                    }
                    Ok(iroh_gossip::api::Event::Lagged) => {
                        tracing::warn!(topic = %topic_name, "gossip receiver lagged on global topic");
                    }
                    Err(err) => {
                        tracing::warn!(error = ?err, topic = %topic_name, "error in global topic receiver");
                    }
                }
            }
            tracing::info!(topic = %topic_name, "global topic receiver loop ended");
        });

        Ok(())
    }

    /// Subscribe to a user-defined topic for public discovery.
    /// Topics are well-known hashes that anyone can subscribe to.
    /// Uses DHT-based auto-discovery to find other peers on the same topic.
    pub async fn subscribe_to_topic(&self, topic_name: &str) -> Result<()> {
        use crate::network::topics::derive_topic_id;

        tracing::info!(topic = %topic_name, "subscribing to user topic");

        // First, subscribe to the standard gossip topic for broadcasting (this is fast)
        let topic_id = TopicId::from_bytes(derive_topic_id(topic_name));
        let broadcaster_topic = self.gossip.subscribe(topic_id, vec![]).await?;
        let receiver_topic = self.gossip.subscribe(topic_id, vec![]).await?;

        // Store the GossipTopic for broadcasting (used by event loop)
        let topic_key = format!("topic:{}", topic_name);
        {
            let mut topics_guard = self.topics.write().await;
            topics_guard.insert(topic_key.clone(), broadcaster_topic);
        }

        // Spawn receiver task for standard gossip
        let inbound_tx = self.inbound_tx.clone();
        let topic_key_for_receiver = topic_key.clone();
        tokio::spawn(async move {
            use futures_util::StreamExt;
            let mut receiver = receiver_topic;
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
                                    tracing::warn!(topic = %topic_key_for_receiver, "inbound channel closed");
                                    break;
                                }
                            }
                            Err(err) => {
                                tracing::warn!(error = ?err, topic = %topic_key_for_receiver, "failed to deserialize message");
                            }
                        }
                    }
                    Ok(iroh_gossip::api::Event::NeighborUp(neighbor_id)) => {
                        tracing::info!(peer = %neighbor_id.fmt_short(), topic = %topic_key_for_receiver, "🎉 peer discovered on topic!");
                    }
                    Ok(iroh_gossip::api::Event::NeighborDown(neighbor_id)) => {
                        tracing::info!(peer = %neighbor_id.fmt_short(), topic = %topic_key_for_receiver, "peer left topic");
                    }
                    Ok(iroh_gossip::api::Event::Lagged) => {
                        tracing::warn!(topic = %topic_key_for_receiver, "gossip receiver lagged");
                    }
                    Err(err) => {
                        tracing::warn!(error = ?err, topic = %topic_key_for_receiver, "error in topic receiver");
                    }
                }
            }
            tracing::info!(topic = %topic_key_for_receiver, "topic receiver loop ended");
        });

        // Spawn DHT auto-discovery in background (this can take a while)
        let gossip = self.gossip.clone();
        let topic_name_owned = topic_name.to_string();
        let inbound_tx_dht = self.inbound_tx.clone();
        let dht_senders = self.dht_senders.clone();

        tokio::spawn(async move {
            use distributed_topic_tracker::AutoDiscoveryGossip;

            tracing::info!(topic = %topic_name_owned, "🔍 starting DHT auto-discovery in background...");

            // Create a deterministic shared secret for this topic
            // This must be the same for all peers on the same topic
            let topic_secret = {
                let mut hasher = blake3::Hasher::new();
                hasher.update(b"graphchan-topic-secret-v1:");
                hasher.update(topic_name_owned.as_bytes());
                hasher.finalize().as_bytes().to_vec()
            };

            // IMPORTANT: Use a UNIQUE signing key for this peer's record in the DHT.
            // If all peers use the same signing key, they will overwrite each other's
            // mutable records (last writer wins), preventing discovery of more than one peer.
            // The dht_topic_id (derived above/below) ensures they all publish to the same
            // "swarm" or location, but each needs their own unique identity signature.
            let topic_signing_key = {
                let mut rng = rand::rng();
                ed25519_dalek::SigningKey::generate(&mut rng)
            };

            // Create the DHT topic ID from the topic name
            let dht_topic_id = DhtTopicId::new(format!("graphchan:{}", topic_name_owned));

            // Create a RecordPublisher for DHT-based peer discovery
            // All peers on the same topic share the same signing key
            let record_publisher = RecordPublisher::new(
                dht_topic_id,
                topic_signing_key.verifying_key(),
                topic_signing_key,
                None, // No secret rotation
                topic_secret,
            );

            // Subscribe with auto-discovery via mainline DHT (can take 30-60+ seconds)
            match gossip.subscribe_and_join_with_auto_discovery(record_publisher).await {
                Ok(dht_topic) => {
                    tracing::info!(topic = %topic_name_owned, "🌐 DHT auto-discovery complete - listening for peers");

                    // Split into sender and receiver for DHT-discovered peers
                    match dht_topic.split().await {
                        Ok((dht_sender, dht_receiver)) => {
                            let topic_key_dht = format!("topic:{}", topic_name_owned);

                            // Store the sender so it can be used for broadcasting
                            {
                                let mut senders = dht_senders.write().await;
                                senders.insert(topic_key_dht.clone(), DhtTopicSender::new(dht_sender));
                                tracing::info!(topic = %topic_key_dht, "📡 DHT sender stored - broadcasts will reach DHT peers");
                            }

                            while let Some(event_result) = dht_receiver.next().await {
                                match event_result {
                                    Ok(iroh_gossip::api::Event::Received(message)) => {
                                        match serde_json::from_slice::<events::EventEnvelope>(&message.content) {
                                            Ok(envelope) => {
                                                let gossip_msg = events::InboundGossip {
                                                    peer_id: Some(message.delivered_from.to_string()),
                                                    payload: envelope.payload,
                                                };
                                                if inbound_tx_dht.send(gossip_msg).await.is_err() {
                                                    break;
                                                }
                                            }
                                            Err(err) => {
                                                tracing::warn!(error = ?err, topic = %topic_key_dht, "failed to deserialize DHT message");
                                            }
                                        }
                                    }
                                    Ok(iroh_gossip::api::Event::NeighborUp(neighbor_id)) => {
                                        tracing::info!(peer = %neighbor_id.fmt_short(), topic = %topic_key_dht, "🎉 DHT peer discovered!");
                                    }
                                    Ok(iroh_gossip::api::Event::NeighborDown(neighbor_id)) => {
                                        tracing::info!(peer = %neighbor_id.fmt_short(), topic = %topic_key_dht, "DHT peer left");
                                    }
                                    Ok(iroh_gossip::api::Event::Lagged) => {
                                        tracing::warn!(topic = %topic_key_dht, "DHT gossip lagged");
                                    }
                                    Err(err) => {
                                        tracing::warn!(error = ?err, topic = %topic_key_dht, "DHT receiver error");
                                        break;
                                    }
                                }
                            }
                        }
                        Err(err) => {
                            tracing::warn!(error = ?err, topic = %topic_name_owned, "failed to split DHT topic");
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!(error = ?err, topic = %topic_name_owned, "❌ DHT auto-discovery failed");
                }
            }
        });

        tracing::info!(topic = %topic_name, "✓ subscribed to topic (DHT discovery running in background)");

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

        // Create two subscriptions to this topic:
        // 1. For receiving (consumed by the spawned task below)
        // 2. For broadcasting (stored in topics map for the event worker)
        let receiver_topic = self.gossip.subscribe(topic_id, bootstrap_peers.clone()).await?;
        let broadcaster_topic = self.gossip.subscribe(topic_id, bootstrap_peers).await?;

        // Store broadcaster in topics map
        {
            let mut topics_guard = self.topics.write().await;
            topics_guard.insert(topic_name.clone(), broadcaster_topic);
        }

        let mut receiver = receiver_topic;
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

        // Get all peers involved in this thread to use as bootstrap nodes
        let bootstrap_peers = self.database.with_repositories(|repos| {
            let mut peer_ids = Vec::new();

            // Get thread creator
            if let Ok(Some(thread)) = repos.threads().get(thread_id) {
                if let Some(creator_id) = thread.creator_peer_id {
                    if let Ok(Some(peer)) = repos.peers().get(&creator_id) {
                        if let Some(iroh_id) = peer.iroh_peer_id {
                            if let Ok(pub_key) = iroh_id.parse::<iroh::PublicKey>() {
                                peer_ids.push(pub_key);
                            }
                        }
                    }
                }
            }

            // Get all post authors
            if let Ok(posts) = repos.posts().list_for_thread(thread_id) {
                for post in posts {
                    if let Some(author_id) = post.author_peer_id {
                        if let Ok(Some(peer)) = repos.peers().get(&author_id) {
                            if let Some(iroh_id) = peer.iroh_peer_id {
                                if let Ok(pub_key) = iroh_id.parse::<iroh::PublicKey>() {
                                    if !peer_ids.contains(&pub_key) {
                                        peer_ids.push(pub_key);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            Ok::<Vec<iroh::PublicKey>, anyhow::Error>(peer_ids)
        }).unwrap_or_default();

        tracing::info!(
            thread_id = %thread_id,
            topic = %topic_name,
            bootstrap_count = bootstrap_peers.len(),
            "subscribing to thread topic for receiving updates"
        );

        // Create two subscriptions to this topic:
        // 1. For receiving (consumed by the spawned task below)
        // 2. For broadcasting (stored in topics map for the event worker)
        let receiver_topic = self.gossip.subscribe(topic_id, bootstrap_peers.clone()).await?;
        let broadcaster_topic = self.gossip.subscribe(topic_id, bootstrap_peers).await?;

        // Store broadcaster in topics map
        {
            let mut topics_guard = self.topics.write().await;
            topics_guard.insert(topic_name.clone(), broadcaster_topic);
        }

        let mut receiver = receiver_topic;
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
                        tracing::info!(peer = %peer_id.fmt_short(), topic = %topic_name, "🤝 neighbor UP on thread topic");
                    }
                    Ok(iroh_gossip::api::Event::NeighborDown(peer_id)) => {
                        tracing::info!(peer = %peer_id.fmt_short(), topic = %topic_name, "👋 neighbor DOWN on thread topic");
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

impl NetworkHandle {
    /// Returns the current DHT connectivity status
    pub fn dht_status(&self) -> DhtStatus {
        if !self.dht_checked.load(Ordering::SeqCst) {
            DhtStatus::Checking
        } else if self.dht_connected.load(Ordering::SeqCst) {
            DhtStatus::Connected
        } else {
            DhtStatus::Unreachable
        }
    }
}

/// Checks if we can connect to the BitTorrent mainline DHT.
/// Returns true if bootstrapped successfully, false otherwise.
async fn check_dht_connectivity() -> Result<bool> {
    // Create a standalone DHT client to test connectivity
    // Dht::client() uses default bootstrap nodes (router.bittorrent.com, dht.transmissionbt.com, etc.)
    let dht = Dht::client().context("failed to create DHT client")?;
    let async_dht = dht.as_async();

    // Wait for bootstrap with a timeout
    let bootstrap_timeout = std::time::Duration::from_secs(30);

    match tokio::time::timeout(bootstrap_timeout, async_dht.bootstrapped()).await {
        Ok(success) => {
            if success {
                // Bootstrap completed successfully - get info for logging
                let info = async_dht.info().await;
                tracing::info!(
                    id = %info.id(),
                    local_addr = ?info.local_addr(),
                    "DHT bootstrapped successfully"
                );
                Ok(true)
            } else {
                tracing::warn!("DHT bootstrap failed - could not reach any bootstrap nodes");
                Ok(false)
            }
        }
        Err(_) => {
            // Timeout - DHT is unreachable
            tracing::warn!("DHT bootstrap timed out after 30s");
            Ok(false)
        }
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
