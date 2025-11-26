use crate::threading::{PostView, ThreadDetails};
use anyhow::Result;
use bytes::Bytes;
use futures_util::StreamExt;
use iroh_blobs::ticket::BlobTicket;
use iroh_gossip::api::GossipTopic;
use iroh_gossip::net::Gossip;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};

type TopicId = iroh_gossip::proto::TopicId;

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
    ProfileUpdate(ProfileUpdate),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAnnouncement {
    pub id: String,
    pub post_id: String,
    pub thread_id: String,
    pub original_name: Option<String>,
    pub mime: Option<String>,
    pub size_bytes: Option<i64>,
    pub checksum: Option<String>,
    pub blob_id: Option<String>,
    pub ticket: Option<BlobTicket>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileUpdate {
    pub peer_id: String,
    pub avatar_file_id: Option<String>,
    pub ticket: Option<BlobTicket>,
    pub username: Option<String>,
    pub bio: Option<String>,
}

#[derive(Debug)]
pub enum NetworkEvent {
    Broadcast(EventPayload),
    Direct {
        #[allow(dead_code)]
        peer_id: String,
        payload: EventPayload,
    },
}

#[derive(Debug, Clone)]
pub struct InboundGossip {
    pub peer_id: Option<String>,
    pub payload: EventPayload,
}

pub async fn run_event_loop(
    gossip: Gossip,
    topics: Arc<RwLock<HashMap<String, GossipTopic>>>,
    mut rx: Receiver<NetworkEvent>,
) {
    tracing::info!("network event loop starting with iroh-gossip");

    while let Some(event) = rx.recv().await {
        match event {
            NetworkEvent::Broadcast(payload) => {
                let topic_name = topic_for_payload(&payload);
                if let Err(err) = broadcast_to_topic(&gossip, &topics, &topic_name, payload).await {
                    tracing::warn!(error = ?err, topic = %topic_name, "failed to broadcast event");
                }
            }
            NetworkEvent::Direct {
                peer_id: _,
                payload,
            } => {
                // iroh-gossip doesn't support direct messaging, so broadcast instead
                let topic_name = topic_for_payload(&payload);
                if let Err(err) = broadcast_to_topic(&gossip, &topics, &topic_name, payload).await {
                    tracing::warn!(error = ?err, topic = %topic_name, "failed to broadcast direct event");
                }
            }
        }
    }

    tracing::info!("network event loop shutting down");
}

async fn broadcast_to_topic(
    gossip: &Gossip,
    topics: &Arc<RwLock<HashMap<String, GossipTopic>>>,
    topic_name: &str,
    payload: EventPayload,
) -> Result<()> {
    let topic_id = TopicId::from_bytes(*blake3::hash(topic_name.as_bytes()).as_bytes());

    // Ensure we're subscribed to this topic and get a mutable reference
    let guard = topics.read().await;
    let needs_subscribe = !guard.contains_key(topic_name);
    drop(guard);

    if needs_subscribe {
        let mut guard = topics.write().await;
        if !guard.contains_key(topic_name) {
            let topic = gossip.subscribe(topic_id, vec![]).await?;
            guard.insert(topic_name.to_string(), topic);
            tracing::debug!(topic = %topic_name, "subscribed to new topic");
        }
    }

    let envelope = envelope_for(payload);
    let bytes = serde_json::to_vec(&envelope)?;

    // Get mutable access to broadcast
    let mut guard = topics.write().await;
    if let Some(topic) = guard.get_mut(topic_name) {
        topic.broadcast(Bytes::from(bytes)).await?;
        tracing::info!(topic = %topic_name, "broadcasted message to topic");
    } else {
        tracing::warn!(topic = %topic_name, "attempted to broadcast to non-existent topic");
    }

    Ok(())
}

pub async fn run_gossip_receiver_loop(
    gossip: Gossip,
    topics: Arc<RwLock<HashMap<String, GossipTopic>>>,
    inbound_tx: Sender<InboundGossip>,
) -> Result<()> {
    // Wait for a peer to be added, which will create the global topic subscription
    tracing::info!("gossip receiver loop waiting for global topic subscription");
    let global_topic_id = TopicId::from_bytes(*blake3::hash(b"graphchan-global").as_bytes());

    let mut receiver = loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Try to get the existing subscription from the topics map
        let guard = topics.read().await;
        if let Some(topic) = guard.get("graphchan-global") {
            // Get a receiver from the existing subscription
            match gossip.subscribe(global_topic_id, vec![]).await {
                Ok(recv) => {
                    drop(guard);
                    break recv;
                }
                Err(_) => {
                    drop(guard);
                    continue;
                }
            }
        }
    };

    tracing::info!("gossip receiver loop started");

    while let Some(event_result) = receiver.next().await {
        match event_result {
            Ok(iroh_gossip::api::Event::Received(message)) => {
                match serde_json::from_slice::<EventEnvelope>(&message.content) {
                    Ok(envelope) => {
                        let peer_id = Some(message.delivered_from.to_string());
                        tracing::info!(
                            from_peer = %message.delivered_from.fmt_short(),
                            topic = %envelope.topic,
                            "received gossip message"
                        );
                        if let Err(err) = inbound_tx
                            .send(InboundGossip {
                                peer_id,
                                payload: envelope.payload,
                            })
                            .await
                        {
                            tracing::warn!(error = ?err, "failed to forward inbound gossip");
                            break;
                        }
                    }
                    Err(err) => {
                        tracing::warn!(error = ?err, "failed to decode gossip envelope");
                    }
                }
            }
            Ok(iroh_gossip::api::Event::NeighborUp(peer_id)) => {
                tracing::info!(peer = %peer_id.fmt_short(), "🎉 GOSSIP NEIGHBOR UP - peer connected to mesh!");
            }
            Ok(iroh_gossip::api::Event::NeighborDown(peer_id)) => {
                tracing::info!(peer = %peer_id.fmt_short(), "❌ GOSSIP NEIGHBOR DOWN");
            }
            Ok(iroh_gossip::api::Event::Lagged) => {
                tracing::warn!("gossip receiver lagged, some messages may have been dropped");
            }
            Err(err) => {
                tracing::error!(error = ?err, "gossip receiver error");
                break;
            }
        }
    }

    tracing::info!("gossip receiver loop ended");
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
        // Route threads, posts, and files to global topic so all connected peers receive them
        EventPayload::ThreadSnapshot(_) => "graphchan-global".to_string(),
        EventPayload::PostUpdate(_) => "graphchan-global".to_string(),
        EventPayload::FileAvailable(_) => "graphchan-global".to_string(),
        // File transfers use dedicated topics
        EventPayload::FileRequest(request) => {
            format!("file-request:{}", request.file_id.clone())
        }
        EventPayload::FileChunk(chunk) => format!("file-chunk:{}", chunk.file_id.clone()),
        EventPayload::ProfileUpdate(_) => "graphchan-global".to_string(),
    }
}
