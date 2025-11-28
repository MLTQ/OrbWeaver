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
use base64::{Engine as _, engine::general_purpose};

type TopicId = iroh_gossip::proto::TopicId;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventEnvelope {
    pub version: u8,
    pub topic: String,
    pub payload: EventPayload,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventPayload {
    ThreadAnnouncement(ThreadAnnouncement),
    PostUpdate(PostView),
    FileAvailable(FileAnnouncement),
    FileRequest(FileRequest),
    FileChunk(FileChunk),
    ProfileUpdate(ProfileUpdate),
}

/// Announces that a thread exists and where to download it.
/// Only broadcast when YOU create/import a thread, not when you download one.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThreadAnnouncement {
    pub thread_id: String,
    pub creator_peer_id: String,
    pub announcer_peer_id: String,  // Who's broadcasting this (may differ from creator)
    pub title: String,
    pub preview: String,             // First ~140 chars of OP body
    pub ticket: BlobTicket,          // Where to download full ThreadDetails
    pub post_count: usize,           // Number of posts (version number)
    pub has_images: bool,
    pub created_at: String,
    pub last_activity: String,       // Most recent post timestamp
    pub thread_hash: String,         // Hash of all post hashes - for sync detection
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
    #[serde(
        serialize_with = "serialize_bytes_as_base64",
        deserialize_with = "deserialize_bytes_from_base64"
    )]
    pub data: Vec<u8>,
    pub eof: bool,
}

fn serialize_bytes_as_base64<S>(data: &Vec<u8>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: serde::Serializer,
{
    let encoded = general_purpose::STANDARD.encode(data);
    serializer.serialize_str(&encoded)
}

fn deserialize_bytes_from_base64<'de, D>(deserializer: D) -> Result<Vec<u8>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let encoded: String = serde::Deserialize::deserialize(deserializer)?;
    general_purpose::STANDARD
        .decode(&encoded)
        .map_err(serde::de::Error::custom)
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

    // Ensure we're subscribed to this topic
    // Note: global topic is created at startup, other topics are auto-created here
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
    let size = bytes.len();

    let payload_type = match &envelope.payload {
        EventPayload::ThreadAnnouncement(_) => "ThreadAnnouncement",
        EventPayload::PostUpdate(_) => "PostUpdate",
        EventPayload::FileAvailable(_) => "FileAnnouncement",
        EventPayload::FileRequest(_) => "FileRequest",
        EventPayload::FileChunk(_) => "FileChunk",
        EventPayload::ProfileUpdate(_) => "ProfileUpdate",
    };

    // Get mutable access to broadcast
    let mut guard = topics.write().await;
    if let Some(topic) = guard.get_mut(topic_name) {
        topic.broadcast(Bytes::from(bytes)).await?;
        tracing::info!(
            topic = %topic_name,
            payload_type = %payload_type,
            size_bytes = size,
            "broadcasted message to topic"
        );
    } else {
        tracing::warn!(topic = %topic_name, "attempted to broadcast to non-existent topic");
    }

    Ok(())
}

pub async fn run_gossip_receiver_loop(
    gossip: Gossip,
    _topics: Arc<RwLock<HashMap<String, GossipTopic>>>,
    inbound_tx: Sender<InboundGossip>,
) -> Result<()> {
    // Subscribe to global topic for receiving
    // Note: This is a separate subscription from the one used for broadcasting
    // iroh-gossip allows multiple subscriptions to the same topic
    let global_topic_id = TopicId::from_bytes(*blake3::hash(b"graphchan-global").as_bytes());
    let mut receiver = gossip.subscribe(global_topic_id, vec![]).await?;

    tracing::info!("gossip receiver loop started");

    while let Some(event_result) = receiver.next().await {
        match event_result {
            Ok(iroh_gossip::api::Event::Received(message)) => {
                let msg_size = message.content.len();
                match serde_json::from_slice::<EventEnvelope>(&message.content) {
                    Ok(envelope) => {
                        let peer_id = Some(message.delivered_from.to_string());
                        let payload_type = match &envelope.payload {
                            EventPayload::ThreadAnnouncement(_) => "ThreadAnnouncement",
                            EventPayload::PostUpdate(_) => "PostUpdate",
                            EventPayload::FileAvailable(_) => "FileAnnouncement",
                            EventPayload::FileRequest(_) => "FileRequest",
                            EventPayload::FileChunk(_) => "FileChunk",
                            EventPayload::ProfileUpdate(_) => "ProfileUpdate",
                        };
                        tracing::info!(
                            from_peer = %message.delivered_from.fmt_short(),
                            topic = %envelope.topic,
                            payload_type = %payload_type,
                            size_bytes = msg_size,
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
                        tracing::warn!(
                            error = ?err,
                            size_bytes = msg_size,
                            "⚠️  failed to decode gossip envelope - message may be too large or corrupted"
                        );
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
        // Announcements go to the announcer's peer topic - only their friends receive it
        EventPayload::ThreadAnnouncement(announcement) => {
            format!("peer-{}", announcement.announcer_peer_id)
        }
        EventPayload::ProfileUpdate(update) => {
            format!("peer-{}", update.peer_id)
        }

        // Thread-specific messages - only sent to peers subscribed to that thread
        EventPayload::PostUpdate(post) => format!("thread-{}", post.thread_id),
        EventPayload::FileAvailable(file) => format!("thread-{}", file.thread_id),

        // These shouldn't be used with the current blob-based file transfer
        EventPayload::FileRequest(_) => "deprecated-file-request".to_string(),
        EventPayload::FileChunk(_) => "deprecated-file-chunk".to_string(),
    }
}
