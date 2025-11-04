use crate::config::{GraphchanPaths, NetworkConfig};
mod events;
mod ingest;

use crate::database::Database;
use crate::identity::{load_iroh_secret, FriendCodePayload};
use crate::threading::{PostView, ThreadDetails};
use anyhow::{Context, Result};
use events::{EventPayload, NetworkEvent};
use iroh::endpoint::{Endpoint, RelayMode};
use iroh_base::{EndpointAddr, PublicKey, RelayUrl};
use iroh_relay::RelayMap;
use std::net::SocketAddr;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio::task::JoinHandle;

const GRAPHCHAN_ALPN: &[u8] = b"graphchan/0";
const GOSSIP_BUFFER: usize = 128;
const INBOUND_BUFFER: usize = 256;

pub use events::FileAnnouncement;

#[derive(Clone)]
pub struct NetworkHandle {
    endpoint: Arc<Endpoint>,
    publisher: mpsc::Sender<events::NetworkEvent>,
    inbound_tx: mpsc::Sender<events::InboundGossip>,
    _event_worker: Arc<JoinHandle<()>>,
    _accept_worker: Arc<JoinHandle<()>>,
    _ingest_worker: Arc<JoinHandle<()>>,
    connections: Arc<RwLock<Vec<events::ConnectionEntry>>>,
}

impl NetworkHandle {
    pub async fn start(
        paths: &GraphchanPaths,
        config: &NetworkConfig,
        database: Database,
    ) -> Result<Self> {
        let secret = load_iroh_secret(paths)?;
        let mut builder = Endpoint::builder()
            .secret_key(secret)
            .alpns(vec![GRAPHCHAN_ALPN.to_vec()]);

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
        endpoint.set_alpns(vec![GRAPHCHAN_ALPN.to_vec()]);
        let endpoint = Arc::new(endpoint);
        let connections = Arc::new(RwLock::new(Vec::new()));

        let (tx, rx) = mpsc::channel(GOSSIP_BUFFER);
        let worker_endpoint = endpoint.clone();
        let worker_config = config.clone();
        let worker_connections = connections.clone();
        let event_worker = tokio::spawn(async move {
            events::run_event_loop(worker_endpoint, worker_config, worker_connections, rx).await;
        });

        let (inbound_tx, inbound_rx) = mpsc::channel(INBOUND_BUFFER);
        let ingest_publisher = tx.clone();
        let ingest_database = database.clone();
        let ingest_paths = paths.clone();
        let ingest_worker = tokio::spawn(async move {
            ingest::run_ingest_loop(ingest_database, ingest_paths, ingest_publisher, inbound_rx)
                .await;
        });

        let accept_endpoint = endpoint.clone();
        let accept_connections = connections.clone();
        let accept_inbound = inbound_tx.clone();
        let accept_worker = tokio::spawn(async move {
            loop {
                match accept_endpoint.accept().await {
                    Some(incoming) => match incoming.accept() {
                        Ok(connecting) => match connecting.await {
                            Ok(connection) => {
                                events::register_connection(
                                    connection,
                                    accept_connections.clone(),
                                    accept_inbound.clone(),
                                )
                                .await;
                            }
                            Err(err) => {
                                tracing::warn!(error = ?err, "failed to finalize incoming connection");
                            }
                        },
                        Err(err) => {
                            tracing::warn!(error = ?err, "failed to accept incoming connection");
                        }
                    },
                    None => break,
                }
            }
            tracing::info!("accept loop shutting down");
        });

        let handle = Self {
            endpoint,
            publisher: tx,
            inbound_tx,
            _event_worker: Arc::new(event_worker),
            _accept_worker: Arc::new(accept_worker),
            _ingest_worker: Arc::new(ingest_worker),
            connections,
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

    pub async fn connect_friendcode(&self, payload: &FriendCodePayload) -> Result<()> {
        let addr = build_endpoint_addr(payload)?;
        match self.endpoint.connect(addr.clone(), GRAPHCHAN_ALPN).await {
            Ok(connection) => {
                tracing::info!(peer = %addr.id.fmt_short(), "connected to peer via friendcode");
                events::register_connection(
                    connection,
                    self.connections.clone(),
                    self.inbound_tx.clone(),
                )
                .await;
            }
            Err(err) => {
                tracing::warn!(error = ?err, peer = %addr.id.fmt_short(), "failed to connect via friendcode");
            }
        }
        Ok(())
    }

    pub async fn shutdown(&self) {
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
