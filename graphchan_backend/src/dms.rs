use crate::config::GraphchanPaths;
use crate::crypto::{decrypt_dm, encrypt_dm, load_x25519_secret};
use crate::database::models::DirectMessageRecord;
use crate::database::repositories::{ConversationRepository, DirectMessageRepository, PeerRepository};
use crate::database::Database;
use crate::utils::now_utc_iso;
use anyhow::{anyhow, Context, Result};
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use x25519_dalek::PublicKey;

#[derive(Clone)]
pub struct DmService {
    database: Database,
    paths: GraphchanPaths,
}

impl DmService {
    pub fn new(database: Database, paths: GraphchanPaths) -> Self {
        Self { database, paths }
    }

    /// Derives a deterministic conversation ID from two peer IDs.
    pub fn derive_conversation_id(peer_a: &str, peer_b: &str) -> String {
        let mut peers = [peer_a, peer_b];
        peers.sort();
        let hash = blake3::hash(format!("orbweaver-dm-v1:{}:{}", peers[0], peers[1]).as_bytes());
        // Convert first 16 bytes to hex string (32 chars)
        hash.as_bytes()[..16]
            .iter()
            .map(|b| format!("{:02x}", b))
            .collect()
    }

    /// Send a direct message to a peer. Returns (view, ciphertext, nonce) for gossip broadcast.
    pub fn send_dm(&self, to_peer_id: &str, body: &str) -> Result<(DirectMessageView, Vec<u8>, Vec<u8>)> {
        // Load our X25519 secret key
        let my_secret = load_x25519_secret(&self.paths)?;

        // Get our own peer ID
        let (my_peer_id, _, _) = self
            .database
            .get_identity()?
            .ok_or_else(|| anyhow!("no local identity found"))?;

        // Get recipient's X25519 public key
        let their_pubkey = self.database.with_repositories(|repos| {
            let peer = repos
                .peers()
                .get(to_peer_id)?
                .ok_or_else(|| anyhow!("peer not found: {}", to_peer_id))?;

            let pubkey_str = peer
                .x25519_pubkey
                .ok_or_else(|| anyhow!("Cannot send DM: peer {} has no X25519 public key. They may have been added via short friendcode. Ask them to share their full friendcode.", to_peer_id))?;

            // Decode base64 public key
            let pubkey_bytes = BASE64_STANDARD.decode(&pubkey_str)
                .with_context(|| "failed to decode X25519 public key")?;

            if pubkey_bytes.len() != 32 {
                anyhow::bail!("invalid X25519 public key length: {}", pubkey_bytes.len());
            }

            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&pubkey_bytes);
            Ok::<PublicKey, anyhow::Error>(PublicKey::from(key_array))
        })?;

        // Encrypt the message
        let (ciphertext, nonce) = encrypt_dm(body, &my_secret.secret, &their_pubkey)?;

        // Derive conversation ID
        let conversation_id = Self::derive_conversation_id(&my_peer_id, to_peer_id);

        // Create message record
        let message_id = Uuid::new_v4().to_string();
        let created_at = now_utc_iso();

        let record = DirectMessageRecord {
            id: message_id.clone(),
            conversation_id: conversation_id.clone(),
            from_peer_id: my_peer_id.clone(),
            to_peer_id: to_peer_id.to_string(),
            encrypted_body: ciphertext.clone(),
            nonce: nonce.to_vec(),
            created_at: created_at.clone(),
            read_at: None,
        };

        let preview: String = body.chars().take(100).collect();
        self.database.with_repositories(|repos| {
            // Store the message
            repos.direct_messages().create(&record)?;

            // Update conversation metadata WITHOUT touching unread_count: replying
            // does not mark the peer's prior unread messages as read.
            repos.conversations().record_outgoing_message(
                &conversation_id,
                to_peer_id,
                &created_at,
                &preview,
            )?;

            Ok(())
        })?;

        let view = DirectMessageView {
            id: message_id,
            conversation_id,
            from_peer_id: my_peer_id,
            to_peer_id: to_peer_id.to_string(),
            body: body.to_string(),
            created_at,
            read_at: None,
        };

        Ok((view, ciphertext, nonce.to_vec()))
    }

    /// Ingest a DM received via gossip. Stores the encrypted record and updates conversation.
    pub fn ingest_dm(&self, from_peer_id: &str, to_peer_id: &str, encrypted_body: &[u8], nonce: &[u8], message_id: &str, conversation_id: &str, created_at: &str) -> Result<()> {
        let record = DirectMessageRecord {
            id: message_id.to_string(),
            conversation_id: conversation_id.to_string(),
            from_peer_id: from_peer_id.to_string(),
            to_peer_id: to_peer_id.to_string(),
            encrypted_body: encrypted_body.to_vec(),
            nonce: nonce.to_vec(),
            created_at: created_at.to_string(),
            read_at: None,
        };

        // Store the raw encrypted record
        self.database.with_repositories(|repos| {
            repos.direct_messages().create(&record)?;
            Ok(())
        })?;

        // Decrypt and update conversation metadata (receive_dm handles conversation upsert)
        if let Err(err) = self.receive_dm(record) {
            tracing::warn!(error = ?err, "failed to decrypt ingested DM for preview");
        }

        Ok(())
    }

    /// Receive and decrypt a direct message.
    pub fn receive_dm(&self, record: DirectMessageRecord) -> Result<DirectMessageView> {
        // Load our X25519 secret key
        let my_secret = load_x25519_secret(&self.paths)?;

        // Get sender's X25519 public key
        let their_pubkey = self.database.with_repositories(|repos| {
            let peer = repos
                .peers()
                .get(&record.from_peer_id)?
                .ok_or_else(|| anyhow!("sender peer not found: {}", record.from_peer_id))?;

            let pubkey_str = peer
                .x25519_pubkey
                .ok_or_else(|| anyhow!("sender {} has no X25519 public key", record.from_peer_id))?;

            // Decode base64 public key
            let pubkey_bytes = BASE64_STANDARD.decode(&pubkey_str)
                .with_context(|| "failed to decode X25519 public key")?;

            if pubkey_bytes.len() != 32 {
                anyhow::bail!("invalid X25519 public key length: {}", pubkey_bytes.len());
            }

            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&pubkey_bytes);
            Ok::<PublicKey, anyhow::Error>(PublicKey::from(key_array))
        })?;

        // Convert nonce Vec<u8> to [u8; 24]
        if record.nonce.len() != 24 {
            anyhow::bail!("invalid nonce length: {}", record.nonce.len());
        }
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&record.nonce);

        // Decrypt the message
        let body = decrypt_dm(&record.encrypted_body, &nonce, &my_secret.secret, &their_pubkey)?;

        // Update conversation metadata: atomic increment of unread_count rather
        // than clobbering it to 1, so receiving multiple unread DMs accumulates
        // correctly.
        let preview: String = body.chars().take(100).collect();
        self.database.with_repositories(|repos| {
            repos.conversations().record_incoming_message(
                &record.conversation_id,
                &record.from_peer_id,
                &record.created_at,
                &preview,
            )?;

            Ok(())
        })?;

        Ok(DirectMessageView {
            id: record.id,
            conversation_id: record.conversation_id,
            from_peer_id: record.from_peer_id,
            to_peer_id: record.to_peer_id,
            body,
            created_at: record.created_at,
            read_at: record.read_at,
        })
    }

    /// List conversations, sorted by last message time.
    pub fn list_conversations(&self) -> Result<Vec<ConversationView>> {
        self.database.with_repositories(|repos| {
            let records = repos.conversations().list()?;
            let mut views = Vec::new();

            for record in records {
                // Get peer info
                if let Some(peer) = repos.peers().get(&record.peer_id)? {
                    views.push(ConversationView {
                        id: record.id,
                        peer_id: record.peer_id,
                        peer_username: peer.username,
                        peer_alias: peer.alias,
                        last_message_at: record.last_message_at,
                        last_message_preview: record.last_message_preview,
                        unread_count: record.unread_count as u32,
                    });
                }
            }

            Ok(views)
        })
    }

    /// Get messages for a specific conversation.
    pub fn get_messages(&self, peer_id: &str, limit: usize) -> Result<Vec<DirectMessageView>> {
        let (my_peer_id, _, _) = self
            .database
            .get_identity()?
            .ok_or_else(|| anyhow!("no local identity found"))?;

        let conversation_id = Self::derive_conversation_id(&my_peer_id, peer_id);

        // Load our X25519 secret key
        let my_secret = load_x25519_secret(&self.paths)?;

        // Get peer's X25519 public key (may not exist for short-friendcode peers)
        let their_pubkey = self.database.with_repositories(|repos| {
            let peer = repos
                .peers()
                .get(peer_id)?
                .ok_or_else(|| anyhow!("peer not found: {}", peer_id))?;

            let pubkey_str = match &peer.x25519_pubkey {
                Some(pk) => pk.clone(),
                None => {
                    // No X25519 key — can't decrypt any messages, return empty
                    return Ok::<Option<PublicKey>, anyhow::Error>(None);
                }
            };

            let pubkey_bytes = BASE64_STANDARD.decode(&pubkey_str)
                .with_context(|| "failed to decode X25519 public key")?;

            if pubkey_bytes.len() != 32 {
                anyhow::bail!("invalid X25519 public key length: {}", pubkey_bytes.len());
            }

            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&pubkey_bytes);
            Ok(Some(PublicKey::from(key_array)))
        })?;

        let their_pubkey = match their_pubkey {
            Some(pk) => pk,
            None => return Ok(Vec::new()), // No X25519 key, no messages can be decrypted
        };

        self.database.with_repositories(|repos| {
            let records = repos.direct_messages().list_for_conversation(&conversation_id, limit)?;
            let mut views = Vec::new();

            for record in records {
                // Convert nonce
                if record.nonce.len() != 24 {
                    tracing::warn!("skipping message with invalid nonce length");
                    continue;
                }
                let mut nonce = [0u8; 24];
                nonce.copy_from_slice(&record.nonce);

                // Decrypt
                match decrypt_dm(&record.encrypted_body, &nonce, &my_secret.secret, &their_pubkey) {
                    Ok(body) => {
                        views.push(DirectMessageView {
                            id: record.id,
                            conversation_id: record.conversation_id,
                            from_peer_id: record.from_peer_id,
                            to_peer_id: record.to_peer_id,
                            body,
                            created_at: record.created_at,
                            read_at: record.read_at,
                        });
                    }
                    Err(e) => {
                        tracing::warn!("failed to decrypt DM {}: {}", record.id, e);
                    }
                }
            }

            Ok(views)
        })
    }

    /// Mark a message as read.
    pub fn mark_as_read(&self, message_id: &str) -> Result<()> {
        let read_at = now_utc_iso();
        self.database.with_repositories(|repos| {
            repos.direct_messages().mark_as_read(message_id, &read_at)?;
            Ok(())
        })
    }

    /// Mark every unread incoming message in a conversation as read in one go,
    /// and reset the conversation's unread_count to zero. Returns the number of
    /// messages updated. Used by the UI when the user opens a conversation —
    /// avoids N round-trips for N unread DMs.
    pub fn mark_conversation_read(&self, peer_id: &str) -> Result<usize> {
        let (my_peer_id, _, _) = self
            .database
            .get_identity()?
            .ok_or_else(|| anyhow!("no local identity found"))?;
        let conversation_id = Self::derive_conversation_id(&my_peer_id, peer_id);
        let read_at = now_utc_iso();

        self.database.with_repositories(|repos| {
            let updated = repos.direct_messages().mark_conversation_read(
                &conversation_id,
                &my_peer_id,
                &read_at,
            )?;
            // Even if 0 rows changed (already-read conversation), normalize the
            // counter to 0 so any earlier inconsistency self-heals.
            repos.conversations().update_unread_count(&conversation_id, 0)?;
            Ok(updated)
        })
    }

    /// Get total unread message count.
    pub fn count_unread(&self) -> Result<usize> {
        let (my_peer_id, _, _) = self
            .database
            .get_identity()?
            .ok_or_else(|| anyhow!("no local identity found"))?;

        self.database.with_repositories(|repos| {
            repos.direct_messages().count_unread(&my_peer_id)
        })
    }
}

/// View model for a direct message with decrypted body.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectMessageView {
    pub id: String,
    pub conversation_id: String,
    pub from_peer_id: String,
    pub to_peer_id: String,
    pub body: String,
    pub created_at: String,
    pub read_at: Option<String>,
}

/// View model for a conversation with peer info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationView {
    pub id: String,
    pub peer_id: String,
    pub peer_username: Option<String>,
    pub peer_alias: Option<String>,
    pub last_message_at: Option<String>,
    pub last_message_preview: Option<String>,
    pub unread_count: u32,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::models::{DirectMessageRecord, PeerRecord};
    use crate::database::repositories::{ConversationRepository, DirectMessageRepository, PeerRepository};
    use rusqlite::Connection;

    fn make_peer(id: &str) -> PeerRecord {
        PeerRecord {
            id: id.into(),
            alias: None,
            username: None,
            bio: None,
            friendcode: None,
            iroh_peer_id: None,
            gpg_fingerprint: None,
            x25519_pubkey: None,
            last_seen: None,
            avatar_file_id: None,
            trust_state: "unknown".into(),
            agents: None,
        }
    }

    fn setup_db() -> Database {
        let db = Database::from_connection(
            Connection::open_in_memory().expect("in-memory db"),
            true,
        );
        db.ensure_migrations().expect("migrations");
        // Pre-seed the peer rows that FK constraints from direct_messages and
        // conversations expect to exist.
        db.with_repositories(|repos| {
            for id in &["alice", "bob", "a", "b"] {
                repos.peers().upsert(&make_peer(id))?;
            }
            Ok(())
        })
        .unwrap();
        db
    }

    fn make_record(id: &str, conv: &str, from: &str, to: &str, ts: &str) -> DirectMessageRecord {
        DirectMessageRecord {
            id: id.into(),
            conversation_id: conv.into(),
            from_peer_id: from.into(),
            to_peer_id: to.into(),
            encrypted_body: vec![1, 2, 3],
            nonce: vec![0u8; 24],
            created_at: ts.into(),
            read_at: None,
        }
    }

    #[test]
    fn test_conversation_id_is_deterministic() {
        let id1 = DmService::derive_conversation_id("alice", "bob");
        let id2 = DmService::derive_conversation_id("bob", "alice");
        assert_eq!(id1, id2);
    }

    #[test]
    fn test_conversation_id_is_unique_per_pair() {
        let id1 = DmService::derive_conversation_id("alice", "bob");
        let id2 = DmService::derive_conversation_id("alice", "charlie");
        assert_ne!(id1, id2);
    }

    #[test]
    fn duplicate_create_is_idempotent() {
        // Re-receiving the same DM (gossip rebroadcast after restart, when the
        // in-memory dedup cache is empty) must not error or produce duplicate rows.
        let db = setup_db();
        let record = make_record("dm-1", "conv-1", "alice", "bob", "2024-01-01T00:00:00Z");

        db.with_repositories(|repos| {
            repos.direct_messages().create(&record)?;
            // Second create with same id: should silently no-op.
            repos.direct_messages().create(&record)?;
            let listed = repos.direct_messages().list_for_conversation("conv-1", 100)?;
            assert_eq!(listed.len(), 1, "duplicate create should not produce two rows");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn record_incoming_message_increments_unread() {
        // Three incoming messages should leave unread_count at 3, not 1.
        let db = setup_db();
        db.with_repositories(|repos| {
            repos.conversations().record_incoming_message(
                "conv-1", "alice", "2024-01-01T00:00:01Z", "hi",
            )?;
            repos.conversations().record_incoming_message(
                "conv-1", "alice", "2024-01-01T00:00:02Z", "again",
            )?;
            repos.conversations().record_incoming_message(
                "conv-1", "alice", "2024-01-01T00:00:03Z", "still here",
            )?;
            let conv = repos.conversations().get("conv-1")?.expect("conv exists");
            assert_eq!(conv.unread_count, 3);
            assert_eq!(conv.last_message_preview.as_deref(), Some("still here"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn outgoing_message_does_not_clear_unread_count() {
        // If we have unread messages from Alice and reply to her, our reply
        // must NOT silently mark her unread messages as read.
        let db = setup_db();
        db.with_repositories(|repos| {
            repos.conversations().record_incoming_message(
                "conv-1", "alice", "2024-01-01T00:00:01Z", "hi",
            )?;
            repos.conversations().record_incoming_message(
                "conv-1", "alice", "2024-01-01T00:00:02Z", "?",
            )?;
            // We reply.
            repos.conversations().record_outgoing_message(
                "conv-1", "alice", "2024-01-01T00:00:03Z", "hey",
            )?;
            let conv = repos.conversations().get("conv-1")?.expect("conv exists");
            assert_eq!(conv.unread_count, 2, "reply must not clear peer's unread count");
            assert_eq!(conv.last_message_preview.as_deref(), Some("hey"));
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn mark_conversation_read_zeroes_unread_and_updates_messages() {
        let db = setup_db();
        let r1 = make_record("dm-1", "conv-1", "alice", "bob", "2024-01-01T00:00:01Z");
        let r2 = make_record("dm-2", "conv-1", "alice", "bob", "2024-01-01T00:00:02Z");
        // Outgoing message — must NOT be marked read by mark_conversation_read.
        let r3 = make_record("dm-3", "conv-1", "bob", "alice", "2024-01-01T00:00:03Z");

        db.with_repositories(|repos| {
            repos.direct_messages().create(&r1)?;
            repos.direct_messages().create(&r2)?;
            repos.direct_messages().create(&r3)?;
            repos.conversations().record_incoming_message(
                "conv-1", "alice", "2024-01-01T00:00:02Z", "?",
            )?;
            // Bob reads the conversation.
            let marked = repos.direct_messages().mark_conversation_read(
                "conv-1", "bob", "2024-01-02T00:00:00Z",
            )?;
            assert_eq!(marked, 2, "should mark only the two incoming messages");
            repos.conversations().update_unread_count("conv-1", 0)?;

            let conv = repos.conversations().get("conv-1")?.expect("conv exists");
            assert_eq!(conv.unread_count, 0);
            // Outgoing message's read_at should still be NULL.
            let outgoing = repos.direct_messages().get("dm-3")?.expect("exists");
            assert!(outgoing.read_at.is_none());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn list_for_conversation_returns_oldest_first() {
        // Inner DESC + LIMIT, outer ASC: when the conversation has more than the
        // limit, we get the most-recent N in oldest-first order.
        let db = setup_db();
        db.with_repositories(|repos| {
            for i in 0..5 {
                let ts = format!("2024-01-01T00:00:{:02}Z", i);
                let id = format!("dm-{}", i);
                repos.direct_messages().create(&make_record(&id, "conv-1", "a", "b", &ts))?;
            }
            // Limit 3 → most recent 3 (dm-2, dm-3, dm-4) in ASC order.
            let listed = repos.direct_messages().list_for_conversation("conv-1", 3)?;
            let ids: Vec<&str> = listed.iter().map(|m| m.id.as_str()).collect();
            assert_eq!(ids, vec!["dm-2", "dm-3", "dm-4"]);
            Ok(())
        })
        .unwrap();
    }
}
