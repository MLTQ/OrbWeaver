use crate::config::GraphchanPaths;
use crate::crypto::{decrypt_dm, encrypt_dm, load_x25519_secret};
use crate::database::models::DirectMessageRecord;
use crate::database::repositories::{
    ConversationRepository, DirectMessageRepository, PeerRepository,
};
use crate::database::Database;
use crate::utils::now_utc_iso;
use anyhow::{anyhow, Context, Result};
use base64::prelude::*;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;
use x25519_dalek::PublicKey;

/// Decrypt-status sentinel values stored in `direct_messages.decrypt_status`.
/// Kept as constants (rather than an enum + serde) so the column is human-
/// readable in sqlite shells and dirt-cheap to query against.
pub const DECRYPT_STATUS_DECRYPTED: &str = "decrypted";
pub const DECRYPT_STATUS_PENDING_KEY: &str = "pending_key";
pub const DECRYPT_STATUS_FAILED: &str = "failed";

/// Conversation preview shown when we have an incoming DM whose sender's
/// x25519 key isn't known yet. Replaced with the real body on a successful
/// retry-decrypt (see `retry_pending_for_sender`).
pub const PENDING_KEY_PREVIEW: &str = "🔒 Encrypted message — sender key not yet known";

/// Categorized failure modes for DM decryption. Lets `ingest_dm` decide whether
/// to surface the message as pending (recoverable), failed (corrupt), or
/// propagate as a genuine error (db / io issue).
#[derive(Debug, Error)]
pub enum DmIngestError {
    /// Sender peer record absent, or peer record exists but has no x25519
    /// pubkey. Recoverable: when the peer's profile arrives via gossip we can
    /// retry decryption from the stored ciphertext.
    #[error("sender x25519 key unknown for peer {peer_id}")]
    MissingKey { peer_id: String },
    /// Cipher / nonce / authentication failure after we had the key. Either
    /// corruption in transit, wrong recipient (somehow routed to us), or a
    /// peer who rotated keys without telling us. Not retried.
    #[error("DM decryption failed: {0}")]
    DecryptFailed(anyhow::Error),
    /// Catch-all for plumbing failures (db lock, key file missing, identity
    /// not loaded, etc.). Bubbles up unchanged.
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

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

    /// Resolve a peer's x25519 public key. Returns `Ok(None)` when the peer
    /// record exists but has no `x25519_pubkey`, OR the peer record is missing
    /// entirely — both are "we should retry once their profile arrives" cases
    /// that the caller needs to distinguish from genuine errors. Returns
    /// `Err` only on db / decoding failures (the latter is genuinely corrupt
    /// data the user can't fix).
    fn lookup_sender_pubkey(&self, peer_id: &str) -> Result<Option<PublicKey>> {
        self.database.with_repositories(|repos| {
            let peer = match repos.peers().get(peer_id)? {
                Some(p) => p,
                None => return Ok(None),
            };
            let Some(pubkey_str) = peer.x25519_pubkey else {
                return Ok(None);
            };

            let pubkey_bytes = BASE64_STANDARD
                .decode(&pubkey_str)
                .with_context(|| "failed to decode X25519 public key")?;
            if pubkey_bytes.len() != 32 {
                anyhow::bail!("invalid X25519 public key length: {}", pubkey_bytes.len());
            }
            let mut key_array = [0u8; 32];
            key_array.copy_from_slice(&pubkey_bytes);
            Ok(Some(PublicKey::from(key_array)))
        })
    }

    /// Send a direct message to a peer. Returns (view, ciphertext, nonce) for gossip broadcast.
    pub fn send_dm(
        &self,
        to_peer_id: &str,
        body: &str,
    ) -> Result<(DirectMessageView, Vec<u8>, Vec<u8>)> {
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
            // Outgoing messages are always "decrypted" — we have the cleartext.
            decrypt_status: DECRYPT_STATUS_DECRYPTED.into(),
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

    /// Ingest a DM received via gossip. Stores the encrypted record and updates
    /// conversation. Decryption failures are categorized:
    ///
    /// - **MissingKey** (sender peer absent or has no x25519): the DM is stored
    ///   with `decrypt_status='pending_key'` and a stub conversation row is
    ///   surfaced with a 🔒 placeholder so the user knows there's pending mail.
    ///   `retry_pending_for_sender()` can re-attempt later when the peer's
    ///   profile arrives via gossip.
    /// - **Failed** (cipher/nonce error after we have the key): unrecoverable —
    ///   stored with `decrypt_status='failed'` and not surfaced. Logged loudly.
    /// - **Decrypted**: existing happy path.
    pub fn ingest_dm(
        &self,
        from_peer_id: &str,
        to_peer_id: &str,
        encrypted_body: &[u8],
        nonce: &[u8],
        message_id: &str,
        conversation_id: &str,
        created_at: &str,
    ) -> Result<()> {
        let record = DirectMessageRecord {
            id: message_id.to_string(),
            conversation_id: conversation_id.to_string(),
            from_peer_id: from_peer_id.to_string(),
            to_peer_id: to_peer_id.to_string(),
            encrypted_body: encrypted_body.to_vec(),
            nonce: nonce.to_vec(),
            created_at: created_at.to_string(),
            read_at: None,
            decrypt_status: DECRYPT_STATUS_DECRYPTED.into(), // optimistic; corrected below
        };

        match self.receive_dm(record.clone()) {
            Ok(_) => {
                // receive_dm succeeded → persist with 'decrypted'.
                self.database.with_repositories(|repos| {
                    repos.direct_messages().create(&record)?;
                    Ok(())
                })?;
            }
            Err(DmIngestError::MissingKey { peer_id }) => {
                tracing::warn!(
                    peer_id = %peer_id,
                    message_id = %record.id,
                    "🔒 DM stored as pending — sender's x25519 key not yet known; will retry on next ProfileUpdate from this peer"
                );
                let mut record = record;
                record.decrypt_status = DECRYPT_STATUS_PENDING_KEY.into();
                self.database.with_repositories(|repos| {
                    repos.direct_messages().create(&record)?;
                    // Stub conversation row so the user sees the activity. Uses
                    // record_incoming_message so unread_count still increments
                    // — the user has new mail even if we can't read it yet.
                    repos.conversations().record_incoming_message(
                        &record.conversation_id,
                        &record.from_peer_id,
                        &record.created_at,
                        PENDING_KEY_PREVIEW,
                    )?;
                    Ok(())
                })?;
            }
            Err(DmIngestError::DecryptFailed(err)) => {
                tracing::error!(
                    error = ?err,
                    message_id = %record.id,
                    from = %record.from_peer_id,
                    "DM cipher/nonce error — message stored as failed and will not be retried"
                );
                let mut record = record;
                record.decrypt_status = DECRYPT_STATUS_FAILED.into();
                self.database.with_repositories(|repos| {
                    repos.direct_messages().create(&record)?;
                    Ok(())
                })?;
            }
            Err(DmIngestError::Other(err)) => return Err(err),
        }

        Ok(())
    }

    /// Re-attempt decryption of every pending-key DM from `from_peer_id`. Called
    /// by the network ingest pipeline when a ProfileUpdate adds (or replaces)
    /// the peer's x25519 key. Returns the number of messages newly transitioned
    /// from 'pending_key' → 'decrypted'.
    ///
    /// On a successful retry, the conversation row's preview is updated with
    /// the most-recently-decrypted message's body (replacing the 🔒 placeholder).
    /// unread_count is left untouched — those messages were already counted as
    /// unread when first received.
    pub fn retry_pending_for_sender(&self, from_peer_id: &str) -> Result<usize> {
        let pending = self.database.with_repositories(|repos| {
            repos
                .direct_messages()
                .list_pending_for_sender(from_peer_id)
        })?;
        if pending.is_empty() {
            return Ok(0);
        }

        let mut decrypted_count = 0usize;
        let mut latest_preview: Option<(String, String)> = None; // (created_at, preview)

        for record in pending {
            match self.receive_dm(record.clone()) {
                Ok(view) => {
                    self.database.with_repositories(|repos| {
                        repos
                            .direct_messages()
                            .update_decrypt_status(&record.id, DECRYPT_STATUS_DECRYPTED)?;
                        Ok(())
                    })?;
                    decrypted_count += 1;
                    let preview: String = view.body.chars().take(100).collect();
                    if latest_preview
                        .as_ref()
                        .map_or(true, |(ts, _)| view.created_at >= *ts)
                    {
                        latest_preview = Some((view.created_at.clone(), preview));
                    }
                }
                Err(DmIngestError::MissingKey { .. }) => {
                    // Still no key — caller will retry again on next ProfileUpdate.
                    continue;
                }
                Err(DmIngestError::DecryptFailed(err)) => {
                    tracing::error!(
                        error = ?err,
                        message_id = %record.id,
                        "pending-key DM failed cipher decode after key arrived — marking failed"
                    );
                    self.database.with_repositories(|repos| {
                        repos
                            .direct_messages()
                            .update_decrypt_status(&record.id, DECRYPT_STATUS_FAILED)?;
                        Ok(())
                    })?;
                }
                Err(DmIngestError::Other(err)) => return Err(err),
            }
        }

        // Replace the 🔒 placeholder preview with the actual most-recent body.
        if decrypted_count > 0 {
            if let Some((ts, preview)) = latest_preview {
                let (my_peer_id, _, _) = self
                    .database
                    .get_identity()?
                    .ok_or_else(|| anyhow!("no local identity found"))?;
                let conversation_id = Self::derive_conversation_id(&my_peer_id, from_peer_id);
                self.database.with_repositories(|repos| {
                    repos
                        .conversations()
                        .update_last_message(&conversation_id, &ts, &preview)?;
                    Ok(())
                })?;
            }
        }

        Ok(decrypted_count)
    }

    /// Receive and decrypt a direct message. Returns categorized errors so the
    /// caller (`ingest_dm` / `retry_pending_for_sender`) can decide whether the
    /// failure is recoverable (`MissingKey`) or terminal (`DecryptFailed`).
    pub fn receive_dm(
        &self,
        record: DirectMessageRecord,
    ) -> std::result::Result<DirectMessageView, DmIngestError> {
        // Load our X25519 secret key (plumbing — bubble as Other).
        let my_secret = load_x25519_secret(&self.paths)?;

        // Get sender's X25519 public key — categorized to MissingKey when the
        // peer or their pubkey is absent so the caller can stash + retry later.
        let their_pubkey = match self.lookup_sender_pubkey(&record.from_peer_id)? {
            Some(pk) => pk,
            None => {
                return Err(DmIngestError::MissingKey {
                    peer_id: record.from_peer_id.clone(),
                });
            }
        };

        // Convert nonce Vec<u8> to [u8; 24]
        if record.nonce.len() != 24 {
            return Err(DmIngestError::DecryptFailed(anyhow!(
                "invalid nonce length: {}",
                record.nonce.len()
            )));
        }
        let mut nonce = [0u8; 24];
        nonce.copy_from_slice(&record.nonce);

        // Decrypt the message
        let body = decrypt_dm(
            &record.encrypted_body,
            &nonce,
            &my_secret.secret,
            &their_pubkey,
        )
        .map_err(DmIngestError::DecryptFailed)?;

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

        // Get peer's X25519 public key. If we don't have one (short friendcode,
        // profile not yet synced), we can't decrypt anything from them — return
        // empty so the UI shows the pending stub conversation rather than an
        // error.
        let their_pubkey = match self.lookup_sender_pubkey(peer_id)? {
            Some(pk) => pk,
            None => return Ok(Vec::new()),
        };

        self.database.with_repositories(|repos| {
            let records = repos
                .direct_messages()
                .list_for_conversation(&conversation_id, limit)?;
            let mut views = Vec::new();

            for record in records {
                // Skip messages that previously failed for non-key reasons. They
                // remain in the DB for forensics but we won't keep retrying.
                if record.decrypt_status == DECRYPT_STATUS_FAILED {
                    continue;
                }

                // Convert nonce
                if record.nonce.len() != 24 {
                    tracing::warn!("skipping message with invalid nonce length");
                    continue;
                }
                let mut nonce = [0u8; 24];
                nonce.copy_from_slice(&record.nonce);

                // Decrypt
                match decrypt_dm(
                    &record.encrypted_body,
                    &nonce,
                    &my_secret.secret,
                    &their_pubkey,
                ) {
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
            repos
                .conversations()
                .update_unread_count(&conversation_id, 0)?;
            Ok(updated)
        })
    }

    /// Get total unread message count.
    pub fn count_unread(&self) -> Result<usize> {
        let (my_peer_id, _, _) = self
            .database
            .get_identity()?
            .ok_or_else(|| anyhow!("no local identity found"))?;

        self.database
            .with_repositories(|repos| repos.direct_messages().count_unread(&my_peer_id))
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
    use crate::database::repositories::{
        ConversationRepository, DirectMessageRepository, PeerRepository,
    };
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
        let db =
            Database::from_connection(Connection::open_in_memory().expect("in-memory db"), true);
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
            decrypt_status: DECRYPT_STATUS_DECRYPTED.into(),
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
            let listed = repos
                .direct_messages()
                .list_for_conversation("conv-1", 100)?;
            assert_eq!(
                listed.len(),
                1,
                "duplicate create should not produce two rows"
            );
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
                "conv-1",
                "alice",
                "2024-01-01T00:00:01Z",
                "hi",
            )?;
            repos.conversations().record_incoming_message(
                "conv-1",
                "alice",
                "2024-01-01T00:00:02Z",
                "again",
            )?;
            repos.conversations().record_incoming_message(
                "conv-1",
                "alice",
                "2024-01-01T00:00:03Z",
                "still here",
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
                "conv-1",
                "alice",
                "2024-01-01T00:00:01Z",
                "hi",
            )?;
            repos.conversations().record_incoming_message(
                "conv-1",
                "alice",
                "2024-01-01T00:00:02Z",
                "?",
            )?;
            // We reply.
            repos.conversations().record_outgoing_message(
                "conv-1",
                "alice",
                "2024-01-01T00:00:03Z",
                "hey",
            )?;
            let conv = repos.conversations().get("conv-1")?.expect("conv exists");
            assert_eq!(
                conv.unread_count, 2,
                "reply must not clear peer's unread count"
            );
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
                "conv-1",
                "alice",
                "2024-01-01T00:00:02Z",
                "?",
            )?;
            // Bob reads the conversation.
            let marked = repos.direct_messages().mark_conversation_read(
                "conv-1",
                "bob",
                "2024-01-02T00:00:00Z",
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
                repos
                    .direct_messages()
                    .create(&make_record(&id, "conv-1", "a", "b", &ts))?;
            }
            // Limit 3 → most recent 3 (dm-2, dm-3, dm-4) in ASC order.
            let listed = repos.direct_messages().list_for_conversation("conv-1", 3)?;
            let ids: Vec<&str> = listed.iter().map(|m| m.id.as_str()).collect();
            assert_eq!(ids, vec!["dm-2", "dm-3", "dm-4"]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn list_pending_for_sender_filters_by_status() {
        // Repo helper for the retry path: only returns 'pending_key' rows from
        // a given sender, ignoring decrypted and failed ones.
        let db = setup_db();
        db.with_repositories(|repos| {
            let mut r1 = make_record("ok-1", "conv-1", "alice", "bob", "2024-01-01T00:00:01Z");
            r1.decrypt_status = DECRYPT_STATUS_DECRYPTED.into();
            let mut r2 = make_record(
                "pending-1",
                "conv-1",
                "alice",
                "bob",
                "2024-01-01T00:00:02Z",
            );
            r2.decrypt_status = DECRYPT_STATUS_PENDING_KEY.into();
            let mut r3 = make_record(
                "pending-2",
                "conv-1",
                "alice",
                "bob",
                "2024-01-01T00:00:03Z",
            );
            r3.decrypt_status = DECRYPT_STATUS_PENDING_KEY.into();
            let mut r4 = make_record("failed-1", "conv-1", "alice", "bob", "2024-01-01T00:00:04Z");
            r4.decrypt_status = DECRYPT_STATUS_FAILED.into();
            // Different sender — must not appear.
            let mut r5 = make_record(
                "other-pending",
                "conv-2",
                "b",
                "bob",
                "2024-01-01T00:00:05Z",
            );
            r5.decrypt_status = DECRYPT_STATUS_PENDING_KEY.into();

            repos.direct_messages().create(&r1)?;
            repos.direct_messages().create(&r2)?;
            repos.direct_messages().create(&r3)?;
            repos.direct_messages().create(&r4)?;
            repos.direct_messages().create(&r5)?;

            let pending = repos.direct_messages().list_pending_for_sender("alice")?;
            let ids: Vec<&str> = pending.iter().map(|m| m.id.as_str()).collect();
            assert_eq!(ids, vec!["pending-1", "pending-2"]);
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn update_decrypt_status_transitions_in_place() {
        let db = setup_db();
        db.with_repositories(|repos| {
            let mut r = make_record("dm-1", "conv-1", "alice", "bob", "2024-01-01T00:00:01Z");
            r.decrypt_status = DECRYPT_STATUS_PENDING_KEY.into();
            repos.direct_messages().create(&r)?;
            repos
                .direct_messages()
                .update_decrypt_status("dm-1", DECRYPT_STATUS_DECRYPTED)?;
            let after = repos.direct_messages().get("dm-1")?.expect("exists");
            assert_eq!(after.decrypt_status, DECRYPT_STATUS_DECRYPTED);
            Ok(())
        })
        .unwrap();
    }
}
