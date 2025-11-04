use crate::config::NetworkConfig;
use crate::threading::{PostView, ThreadDetails};
use anyhow::Result;
use iroh::endpoint::{Connection, Endpoint, RecvStream};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: u8,
    pub topic: String,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    ThreadSnapshot(ThreadDetails),
    PostUpdate(PostView),
    FileAvailable(FileAnnouncement),
    FileRequest(FileRequest),
    FileChunk(FileChunk),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnnouncement {
    pub id: String,
    pub post_id: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub blob_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRequest {
    pub file_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChunk {
    pub file_id: String,
    pub data: Vec<u8>,
    pub eof: bool,
}

#[derive(Debug)]
pub enum NetworkEvent {
    Broadcast(EventPayload),
    Direct {
        peer_id: String,
        payload: EventPayload,
    },
}

#[derive(Clone)]
pub struct ConnectionEntry {
    pub id: Uuid,
    pub connection: Arc<Connection>,
    pub peer_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct InboundGossip {
    pub peer_id: Option<String>,
    pub payload: EventPayload,
}

pub async fn run_event_loop(
    _endpoint: Arc<Endpoint>,
    config: NetworkConfig,
    connections: Arc<RwLock<Vec<ConnectionEntry>>>,
    mut rx: Receiver<NetworkEvent>,
) {
    if let Some(relay) = config.relay_url.as_deref() {
        tracing::info!(relay, "network event loop starting with relay");
    } else {
        tracing::info!("network event loop starting without relay override");
    }

    while let Some(event) = rx.recv().await {
        match event {
            NetworkEvent::Broadcast(payload) => {
                let envelope = envelope_for(payload);
                let bytes = match serde_json::to_vec(&envelope) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        tracing::warn!(error = ?err, "failed to serialize network event");
                        continue;
                    }
                };

                let targets: Vec<(Uuid, Arc<Connection>)> = {
                    let guard = connections.read().await;
                    guard
                        .iter()
                        .map(|entry| (entry.id, entry.connection.clone()))
                        .collect()
                };

                prune_failed(targets, &bytes, &connections).await;
            }
            NetworkEvent::Direct { peer_id, payload } => {
                let envelope = envelope_for(payload);
                let bytes = match serde_json::to_vec(&envelope) {
                    Ok(bytes) => bytes,
                    Err(err) => {
                        tracing::warn!(error = ?err, ?peer_id, "failed to serialize direct network event");
                        continue;
                    }
                };

                let targets: Vec<(Uuid, Arc<Connection>)> = {
                    let guard = connections.read().await;
                    guard
                        .iter()
                        .filter(|entry| entry.peer_id.as_deref() == Some(&peer_id))
                        .map(|entry| (entry.id, entry.connection.clone()))
                        .collect()
                };

                if targets.is_empty() {
                    tracing::debug!(peer_id, "no active connection for direct event");
                    continue;
                }

                prune_failed(targets, &bytes, &connections).await;
            }
        }
    }

    tracing::info!("network event loop shutting down");
}

pub async fn register_connection(
    connection: Connection,
    connections: Arc<RwLock<Vec<ConnectionEntry>>>,
    inbound: Sender<InboundGossip>,
) {
    let connection = Arc::new(connection);
    let entry_id = Uuid::new_v4();
    let peer_id = connection.remote_id().ok().map(|id| id.to_string());
    {
        let mut guard = connections.write().await;
        guard.push(ConnectionEntry {
            id: entry_id,
            connection: connection.clone(),
            peer_id: peer_id.clone(),
        });
    }

    let remote = peer_id.clone().unwrap_or_else(|| "unknown".into());
    tracing::info!(%remote, "iroh connection established");

    let reader_connection = connection.clone();
    let reader_inbound = inbound.clone();
    let reader_remote = peer_id.clone();
    tokio::spawn(async move {
        if let Err(err) =
            pump_inbound_streams(reader_connection, reader_inbound, reader_remote).await
        {
            tracing::warn!(error = ?err, "failed to read inbound gossip stream");
        }
    });

    tokio::spawn(async move {
        let _ = connection.closed().await;
        tracing::info!(%remote, "iroh connection closed");
        let mut guard = connections.write().await;
        guard.retain(|entry| entry.id != entry_id);
    });
}

async fn send_event(connection: &Connection, bytes: &[u8]) -> Result<()> {
    let mut stream = connection.open_uni().await?;
    let len = bytes.len() as u32;
    stream.write_all(&len.to_be_bytes()).await?;
    stream.write_all(bytes).await?;
    stream.finish()?;
    Ok(())
}

fn envelope_for(payload: EventPayload) -> EventEnvelope {
    let topic = topic_for_payload(&payload);
    EventEnvelope {
        version: 1,
        topic,
        payload,
    }
}

fn topic_for_payload(payload: &EventPayload) -> String {
    match payload {
        EventPayload::ThreadSnapshot(snapshot) => format!("thread:{}", snapshot.thread.id.clone()),
        EventPayload::PostUpdate(post) => format!("thread:{}", post.thread_id.clone()),
        EventPayload::FileAvailable(announcement) => {
            format!("file:{}", announcement.id.clone())
        }
        EventPayload::FileRequest(request) => {
            format!("file-request:{}", request.file_id.clone())
        }
        EventPayload::FileChunk(chunk) => format!("file-chunk:{}", chunk.file_id.clone()),
    }
}

async fn prune_failed(
    targets: Vec<(Uuid, Arc<Connection>)>,
    bytes: &[u8],
    connections: &Arc<RwLock<Vec<ConnectionEntry>>>,
) {
    let mut failed = Vec::new();
    for (id, connection) in targets {
        if let Err(err) = send_event(&connection, bytes).await {
            let remote = connection
                .remote_id()
                .map(|id| id.to_string())
                .unwrap_or_else(|_| "unknown".into());
            tracing::warn!(error = ?err, %remote, "failed to deliver network event");
            failed.push(id);
        }
    }

    if !failed.is_empty() {
        let mut guard = connections.write().await;
        guard.retain(|entry| !failed.contains(&entry.id));
    }
}

async fn pump_inbound_streams(
    connection: Arc<Connection>,
    inbound: Sender<InboundGossip>,
    peer_id: Option<String>,
) -> Result<()> {
    loop {
        match connection.accept_uni().await {
            Ok(mut stream) => match receive_event(&mut stream).await {
                Ok(payload) => {
                    if inbound
                        .send(InboundGossip {
                            peer_id: peer_id.clone(),
                            payload,
                        })
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(err) => {
                    tracing::warn!(error = ?err, "failed to decode inbound gossip payload");
                }
            },
            Err(err) => {
                tracing::debug!(error = ?err, "connection accept_uni failed, stopping reader");
                break;
            }
        }
    }
    Ok(())
}

async fn receive_event(stream: &mut RecvStream) -> Result<EventPayload> {
    let mut len_buf = [0u8; 4];
    stream.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;
    let envelope: EventEnvelope = serde_json::from_slice(&payload)?;
    Ok(envelope.payload)
}
