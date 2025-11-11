use crate::config::{GraphchanPaths, NetworkConfig};
mod events;
mod ingest;

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

type TopicId = iroh_gossip::proto::TopicId;

#[derive(Clone)]
pub struct NetworkHandle {
    endpoint: Arc<Endpoint>,
    gossip: Gossip,
    publisher: mpsc::Sender<events::NetworkEvent>,
    _event_worker: Arc<JoinHandle<()>>,
    _ingest_worker: Arc<JoinHandle<()>>,
    _router: Arc<Router>,
    topics: Arc<RwLock<HashMap<String, GossipTopic>>>,
    _blob_store: FsStore,
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
        let event_worker_topics_clone = event_worker_topics.clone();
        let event_worker = tokio::spawn(async move {
            events::run_event_loop(event_worker_gossip, event_worker_topics_clone, rx).await;
        });

        let (inbound_tx, inbound_rx) = mpsc::channel(GOSSIP_BUFFER);
        let ingest_publisher = tx.clone();
        let ingest_database = database.clone();
        let ingest_paths = paths.clone();
        let ingest_store = blob_store.clone();
        let ingest_worker = tokio::spawn(async move {
            ingest::run_ingest_loop(
                ingest_database,
                ingest_paths,
                ingest_publisher,
                inbound_rx,
                ingest_store,
            )
            .await;
        });

        let gossip_receiver_worker_gossip = gossip.clone();
        let gossip_receiver_worker_tx = inbound_tx.clone();
        tokio::spawn(async move {
            if let Err(err) = events::run_gossip_receiver_loop(
                gossip_receiver_worker_gossip,
                gossip_receiver_worker_tx,
            )
            .await
            {
                tracing::error!(error = ?err, "gossip receiver loop failed");
            }
        });

        let blob_protocol = BlobsProtocol::new(&blob_store, None);
        let router = Router::builder(endpoint.as_ref().clone())
            .accept(BLOBS_ALPN, blob_protocol)
            .spawn();
        let router = Arc::new(router);

        let handle = Self {
            endpoint,
            gossip,
            publisher: tx,
            _event_worker: Arc::new(event_worker),
            _ingest_worker: Arc::new(ingest_worker),
            _router: router.clone(),
            topics: event_worker_topics,
            _blob_store: blob_store.clone(),
        };
        tracing::info!(peer_id = %handle.peer_id(), "iroh endpoint started");
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

    /// Broadcasts a full thread snapshot (including posts) to connected peers.
    pub async fn publish_thread_snapshot(&self, snapshot: ThreadDetails) -> Result<()> {
        let event = NetworkEvent::Broadcast(EventPayload::ThreadSnapshot(snapshot));
        self.publisher.send(event).await.ok();
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

    /// Returns the list of currently connected peer IDs.
    pub async fn connected_peer_ids(&self) -> Vec<String> {
        // For iroh-gossip, we can't easily query all neighbors from all topics
        // without consuming the receivers. Return empty for now.
        // In a real implementation, you'd track this separately or use a different approach.
        Vec::new()
    }

    pub async fn connect_friendcode(&self, payload: &FriendCodePayload) -> Result<()> {
        let addr = build_endpoint_addr(payload)?;
        let peer_id = addr.id;

        // Join the global gossip topic with this peer as a bootstrap node
        let global_topic_id = TopicId::from_bytes(*blake3::hash(b"graphchan-global").as_bytes());

        // Subscribe with this peer as bootstrap if not already subscribed
        let mut guard = self.topics.write().await;
        if !guard.contains_key("graphchan-global") {
            let topic = self
                .gossip
                .subscribe(global_topic_id, vec![peer_id])
                .await?;
            guard.insert("graphchan-global".to_string(), topic);
            tracing::info!(peer = %peer_id.fmt_short(), "subscribed to global topic with peer as bootstrap");
        } else {
            tracing::info!(peer = %peer_id.fmt_short(), "already subscribed to global topic");
        }

        Ok(())
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
