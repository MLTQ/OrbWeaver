use crate::config::GraphchanPaths;
use crate::crypto::{decrypt_dm, encrypt_dm, load_x25519_secret};
use crate::database::models::{ConversationRecord, DirectMessageRecord};
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

        self.database.with_repositories(|repos| {
            // Store the message
            repos.direct_messages().create(&record)?;

            // Update conversation metadata
            let conversation = ConversationRecord {
                id: conversation_id.clone(),
                peer_id: to_peer_id.to_string(),
                last_message_at: Some(created_at.clone()),
                last_message_preview: Some(body.chars().take(100).collect()),
                unread_count: 0, // We sent it, so it's not unread for us
            };
            repos.conversations().upsert(&conversation)?;

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

        // Update conversation metadata
        let (my_peer_id, _, _) = self
            .database
            .get_identity()?
            .ok_or_else(|| anyhow!("no local identity found"))?;

        self.database.with_repositories(|repos| {
            // Update conversation
            let conversation = ConversationRecord {
                id: record.conversation_id.clone(),
                peer_id: record.from_peer_id.clone(),
                last_message_at: Some(record.created_at.clone()),
                last_message_preview: Some(body.chars().take(100).collect()),
                unread_count: 1, // New unread message
            };
            repos.conversations().upsert(&conversation)?;

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
}
