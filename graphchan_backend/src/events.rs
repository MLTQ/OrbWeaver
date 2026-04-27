//! Live event stream broadcast to API consumers (SSE).
//!
//! The ingest loop publishes a typed `AppEvent` after each successful
//! gossip-driven mutation; HTTP clients subscribed to `/events` receive the
//! same stream as Server-Sent Events. Used by agents that want to react to
//! activity without polling.
//!
//! The broadcast uses `tokio::sync::broadcast`, which drops messages for slow
//! consumers (they receive `Lagged` and the SSE handler emits a synthetic
//! `lagged` event so clients can decide whether to resync).

use serde::Serialize;
use tokio::sync::broadcast;

/// Channel capacity for the live event broadcast. Each subscribed SSE client
/// gets its own ring buffer of this size; slow clients lag, they don't block.
pub const EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Clone, Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AppEvent {
    /// A new (or updated) post was applied to a thread.
    PostAdded {
        thread_id: String,
        post_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        author_peer_id: Option<String>,
    },
    /// A thread announcement was received from a peer.
    ThreadAnnounced {
        thread_id: String,
        title: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        creator_peer_id: Option<String>,
    },
    /// A file announcement was received (file may not yet be downloaded).
    FileAnnounced {
        file_id: String,
        post_id: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        size_bytes: Option<i64>,
    },
    /// A blob finished downloading and is now present locally.
    FileDownloaded { file_id: String, post_id: String },
    /// A peer's profile (username/bio/avatar) was updated.
    ProfileUpdated { peer_id: String },
    /// A reaction was added or removed.
    ReactionUpdated {
        post_id: String,
        reactor_peer_id: String,
        emoji: String,
        removed: bool,
    },
    /// A direct message was received and decrypted.
    DmReceived {
        from_peer_id: String,
        conversation_id: String,
        message_id: String,
    },
}

/// A typed handle around the broadcast sender used by the ingest pipeline.
/// Sending on a closed channel is silently dropped (no subscribers is normal).
#[derive(Clone)]
pub struct EventPublisher {
    tx: broadcast::Sender<AppEvent>,
}

impl EventPublisher {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self { tx }
    }

    pub fn publish(&self, event: AppEvent) {
        // Send returns Err only when there are zero subscribers, which is fine.
        let _ = self.tx.send(event);
    }

    pub fn subscribe(&self) -> broadcast::Receiver<AppEvent> {
        self.tx.subscribe()
    }
}

impl Default for EventPublisher {
    fn default() -> Self {
        Self::new()
    }
}
