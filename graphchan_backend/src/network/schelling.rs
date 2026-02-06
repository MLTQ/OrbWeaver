//! Schelling Point Discovery for Topic-Based Peer Finding
//!
//! Two strangers who both subscribe to the same topic (e.g., "cats") derive
//! identical BEP44 signing keys from the topic name + time window. Each peer
//! publishes an encrypted record containing their full EndpointAddr (node ID +
//! relay URL + direct addresses) to the DHT. Other peers on the same topic
//! query the DHT, decrypt the records, and inject discovered addresses into
//! iroh's StaticProvider for direct connection.
//!
//! Key properties:
//! - Same topic + same minute → same BEP44 (public_key, salt) → discoverable
//! - Records encrypted with ChaCha20Poly1305 so passive DHT observers can't
//!   read addresses (only peers who know the topic name can decrypt)
//! - Per-minute key rotation limits DHT pollution; old records expire naturally
//! - Last-writer-wins on the same (key, salt) slot is acceptable because we
//!   query both current and previous minute windows and republish every ~30s

use anyhow::{Context, Result};
use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Nonce,
};
use hkdf::Hkdf;
use iroh::discovery::static_provider::StaticProvider;
use iroh::endpoint::Endpoint;
use iroh_base::{EndpointAddr, PublicKey, RelayUrl};
use iroh_gossip::net::Gossip;
use mainline::{MutableItem, SigningKey};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha512};
use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

/// A record published to the DHT containing our endpoint addressing info.
/// Serialized as JSON, then encrypted before BEP44 storage.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct SchellingRecord {
    /// iroh PublicKey bytes (32 bytes, hex-encoded for JSON safety)
    node_id: String,
    /// Relay URL if connected to one (from endpoint.addr())
    relay_url: Option<String>,
    /// Direct socket addresses (STUN-discovered, public IP, etc.)
    direct_addrs: Vec<String>,
}

/// Derives the SHA-512 hash of a topic name, used as root material for
/// both the BEP44 signing key and the encryption key.
fn topic_hash(topic_name: &str) -> [u8; 64] {
    let mut hasher = Sha512::new();
    hasher.update(b"topic:");
    hasher.update(topic_name.as_bytes());
    let result = hasher.finalize();
    let mut hash = [0u8; 64];
    hash.copy_from_slice(&result);
    hash
}

/// Derives the BEP44 signing keypair for a given topic + minute window.
/// All peers on the same topic in the same minute derive the same keypair.
fn derive_signing_key(topic_name: &str, minute: u64) -> SigningKey {
    let hash = topic_hash(topic_name);
    let ikm = &hash[..32];
    let info = minute.to_le_bytes();

    let hk = Hkdf::<sha2::Sha256>::new(None, ikm);
    let mut seed = [0u8; 32];
    hk.expand(&info, &mut seed)
        .expect("32 bytes is valid HKDF-SHA256 output length");

    SigningKey::from_bytes(&seed)
}

/// Derives the BEP44 salt from the topic name (blake3 hash).
fn derive_salt(topic_name: &str) -> Vec<u8> {
    blake3::hash(topic_name.as_bytes()).as_bytes().to_vec()
}

/// Derives the ChaCha20Poly1305 encryption key from the topic hash.
/// Uses the second half of the SHA-512 topic hash as input key material.
fn derive_encryption_key(topic_name: &str) -> [u8; 32] {
    let hash = topic_hash(topic_name);
    let ikm = &hash[32..64];

    let hk = Hkdf::<sha2::Sha256>::new(None, ikm);
    let mut key = [0u8; 32];
    hk.expand(b"schelling-encrypt", &mut key)
        .expect("32 bytes is valid HKDF-SHA256 output length");
    key
}

/// Encrypts a SchellingRecord for DHT storage.
/// Format: 12-byte nonce || ciphertext (same pattern as encrypt_thread_blob).
fn encrypt_record(plaintext: &[u8], encryption_key: &[u8; 32]) -> Result<Vec<u8>> {
    let cipher = ChaCha20Poly1305::new(encryption_key.into());
    let nonce_bytes = crate::crypto::generate_nonce_12();
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|e| anyhow::anyhow!("schelling record encryption failed: {}", e))?;

    let mut result = nonce_bytes.to_vec();
    result.extend(ciphertext);
    Ok(result)
}

/// Decrypts a SchellingRecord from DHT storage.
fn decrypt_record(encrypted: &[u8], encryption_key: &[u8; 32]) -> Result<Vec<u8>> {
    if encrypted.len() < 12 {
        anyhow::bail!("schelling record too short to contain nonce");
    }

    let (nonce_bytes, ciphertext) = encrypted.split_at(12);
    let nonce = Nonce::from_slice(nonce_bytes);
    let cipher = ChaCha20Poly1305::new(encryption_key.into());

    cipher
        .decrypt(nonce, ciphertext)
        .map_err(|e| anyhow::anyhow!("schelling record decryption failed: {}", e))
}

/// Returns the current Unix minute (timestamp / 60).
fn current_minute() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before epoch")
        .as_secs()
        / 60
}

/// Builds a SchellingRecord from the current endpoint state.
fn build_own_record(endpoint: &Endpoint) -> SchellingRecord {
    let addr = endpoint.addr();
    let node_id = endpoint.id().to_string();

    let relay_url = addr.relay_urls().next().map(|r| r.to_string());

    let direct_addrs: Vec<String> = addr.ip_addrs().map(|a| a.to_string()).collect();

    SchellingRecord {
        node_id,
        relay_url,
        direct_addrs,
    }
}

/// Converts a SchellingRecord into an iroh EndpointAddr for StaticProvider injection.
fn record_to_endpoint_addr(record: &SchellingRecord) -> Result<EndpointAddr> {
    let pub_key: PublicKey = record
        .node_id
        .parse()
        .context("invalid node_id in schelling record")?;

    let mut addr = EndpointAddr::new(pub_key);

    if let Some(relay_str) = &record.relay_url {
        if let Ok(relay) = relay_str.parse::<RelayUrl>() {
            addr = addr.with_relay_url(relay);
        }
    }

    for direct_str in &record.direct_addrs {
        if let Ok(sock) = direct_str.parse::<SocketAddr>() {
            addr = addr.with_ip_addr(sock);
        }
    }

    Ok(addr)
}

/// Publishes our record to a specific minute slot via BEP44.
fn publish_record(
    dht: &mainline::Dht,
    topic_name: &str,
    minute: u64,
    record: &SchellingRecord,
    encryption_key: &[u8; 32],
) -> Result<()> {
    let signing_key = derive_signing_key(topic_name, minute);
    let salt = derive_salt(topic_name);

    let plaintext = serde_json::to_vec(record).context("failed to serialize schelling record")?;
    let encrypted = encrypt_record(&plaintext, encryption_key)?;

    // BEP44 value size limit is 1000 bytes. Check before publishing.
    if encrypted.len() > 1000 {
        anyhow::bail!(
            "schelling record too large for BEP44: {} bytes (max 1000)",
            encrypted.len()
        );
    }

    // mainline uses ed25519_dalek::SigningKey internally, same type
    let item = MutableItem::new(signing_key, &encrypted, minute as i64, Some(&salt));

    dht.put_mutable(item, None)
        .map_err(|e| anyhow::anyhow!("BEP44 put_mutable failed: {:?}", e))?;

    Ok(())
}

/// Queries a specific minute slot via BEP44 and returns decoded records.
fn query_records(
    dht: &mainline::Dht,
    topic_name: &str,
    minute: u64,
    encryption_key: &[u8; 32],
) -> Vec<SchellingRecord> {
    let signing_key = derive_signing_key(topic_name, minute);
    let public_key = signing_key.verifying_key().to_bytes();
    let salt = derive_salt(topic_name);

    let mut records = Vec::new();

    let results = dht.get_mutable(&public_key, Some(&salt), None);

    for item in results {
        match decrypt_record(item.value(), encryption_key) {
            Ok(plaintext) => match serde_json::from_slice::<SchellingRecord>(&plaintext) {
                Ok(record) => {
                    records.push(record);
                }
                Err(err) => {
                    tracing::trace!(error = ?err, "failed to deserialize schelling record");
                }
            },
            Err(err) => {
                tracing::trace!(error = ?err, "failed to decrypt schelling record");
            }
        }
    }

    records
}

/// Main discovery loop. Publishes our endpoint info and discovers peers on the
/// same topic via BEP44 shared keys. Runs until the task is cancelled.
pub async fn run_schelling_loop(
    topic_name: String,
    endpoint: Arc<Endpoint>,
    gossip: Gossip,
    static_provider: StaticProvider,
) {
    tracing::info!(topic = %topic_name, "starting schelling point discovery loop");

    // Create a blocking DHT client (mainline operations are blocking)
    let dht = match mainline::Dht::client() {
        Ok(d) => d,
        Err(err) => {
            tracing::error!(error = ?err, topic = %topic_name, "failed to create DHT client for schelling discovery");
            return;
        }
    };

    let encryption_key = derive_encryption_key(&topic_name);
    let our_node_id = endpoint.id().to_string();

    // Track which peers we've already injected to avoid redundant work
    let mut known_peers: HashSet<String> = HashSet::new();

    // Derive the gossip topic ID (same as subscribe_to_topic uses)
    let gossip_topic_id = iroh_gossip::proto::TopicId::from_bytes(
        crate::network::topics::derive_topic_id(&topic_name),
    );

    loop {
        let minute = current_minute();

        // --- Query phase: look for peers in current and previous minute slots ---
        let mut discovered = Vec::new();
        for query_minute in [minute, minute.saturating_sub(1)] {
            // Run blocking DHT query on a blocking thread
            let topic_clone = topic_name.clone();
            let enc_key = encryption_key;
            let dht_ref = &dht;

            let records = tokio::task::block_in_place(|| {
                query_records(dht_ref, &topic_clone, query_minute, &enc_key)
            });

            discovered.extend(records);
        }

        // --- Process discovered peers ---
        for record in &discovered {
            // Skip our own records
            if record.node_id == our_node_id {
                continue;
            }

            let is_new = known_peers.insert(record.node_id.clone());

            match record_to_endpoint_addr(record) {
                Ok(addr) => {
                    // Inject into StaticProvider so iroh can resolve this peer
                    static_provider.add_endpoint_info(addr.clone());

                    if is_new {
                        tracing::info!(
                            peer = %record.node_id,
                            relay = ?record.relay_url,
                            direct_addrs = record.direct_addrs.len(),
                            topic = %topic_name,
                            "schelling: discovered new peer, injected into static provider"
                        );

                        // Add as bootstrap peer to the gossip topic so we mesh with them
                        if let Ok(pub_key) = record.node_id.parse::<PublicKey>() {
                            if let Err(err) = gossip
                                .subscribe(gossip_topic_id, vec![pub_key])
                                .await
                            {
                                tracing::warn!(
                                    error = ?err,
                                    peer = %record.node_id,
                                    topic = %topic_name,
                                    "schelling: failed to add peer to gossip topic"
                                );
                            }
                        }
                    } else {
                        // Update addresses for known peer (they may have changed relay/IP)
                        tracing::trace!(
                            peer = %record.node_id,
                            topic = %topic_name,
                            "schelling: refreshed peer addresses"
                        );
                    }
                }
                Err(err) => {
                    tracing::warn!(
                        error = ?err,
                        peer = %record.node_id,
                        topic = %topic_name,
                        "schelling: failed to parse peer endpoint address"
                    );
                }
            }
        }

        // --- Publish phase: advertise our own record ---
        let own_record = build_own_record(&endpoint);
        let topic_clone = topic_name.clone();
        let enc_key = encryption_key;
        let dht_ref = &dht;

        let publish_result = tokio::task::block_in_place(|| {
            publish_record(dht_ref, &topic_clone, minute, &own_record, &enc_key)
        });

        match publish_result {
            Ok(()) => {
                tracing::debug!(
                    topic = %topic_name,
                    minute = minute,
                    relay = ?own_record.relay_url,
                    direct_addrs = own_record.direct_addrs.len(),
                    discovered_peers = discovered.len().saturating_sub(1), // exclude self
                    "schelling: published record and completed discovery cycle"
                );
            }
            Err(err) => {
                tracing::warn!(
                    error = ?err,
                    topic = %topic_name,
                    "schelling: failed to publish BEP44 record"
                );
            }
        }

        // Sleep 30 seconds before next cycle
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation_deterministic() {
        let key1 = derive_signing_key("cats", 12345);
        let key2 = derive_signing_key("cats", 12345);

        assert_eq!(key1.to_bytes(), key2.to_bytes());
    }

    #[test]
    fn test_key_derivation_different_topics() {
        let key1 = derive_signing_key("cats", 12345);
        let key2 = derive_signing_key("dogs", 12345);

        assert_ne!(key1.to_bytes(), key2.to_bytes());
    }

    #[test]
    fn test_key_derivation_different_minutes() {
        let key1 = derive_signing_key("cats", 12345);
        let key2 = derive_signing_key("cats", 12346);

        assert_ne!(key1.to_bytes(), key2.to_bytes());
    }

    #[test]
    fn test_encryption_roundtrip() {
        let key = derive_encryption_key("test-topic");
        let plaintext = b"hello world";

        let encrypted = encrypt_record(plaintext, &key).unwrap();
        let decrypted = decrypt_record(&encrypted, &key).unwrap();

        assert_eq!(plaintext.as_ref(), decrypted.as_slice());
    }

    #[test]
    fn test_encryption_wrong_key_fails() {
        let key1 = derive_encryption_key("topic-a");
        let key2 = derive_encryption_key("topic-b");
        let plaintext = b"secret data";

        let encrypted = encrypt_record(plaintext, &key1).unwrap();
        let result = decrypt_record(&encrypted, &key2);

        assert!(result.is_err());
    }

    #[test]
    fn test_salt_deterministic() {
        let salt1 = derive_salt("cats");
        let salt2 = derive_salt("cats");

        assert_eq!(salt1, salt2);
    }

    #[test]
    fn test_salt_different_topics() {
        let salt1 = derive_salt("cats");
        let salt2 = derive_salt("dogs");

        assert_ne!(salt1, salt2);
    }

    #[test]
    fn test_encryption_key_deterministic() {
        let key1 = derive_encryption_key("cats");
        let key2 = derive_encryption_key("cats");

        assert_eq!(key1, key2);
    }

    #[test]
    fn test_encryption_key_different_topics() {
        let key1 = derive_encryption_key("cats");
        let key2 = derive_encryption_key("dogs");

        assert_ne!(key1, key2);
    }

    #[test]
    fn test_record_serialization_roundtrip() {
        let record = SchellingRecord {
            node_id: "abc123".to_string(),
            relay_url: Some("https://relay.example.com".to_string()),
            direct_addrs: vec!["192.168.1.1:8080".to_string()],
        };

        let json = serde_json::to_vec(&record).unwrap();
        let decoded: SchellingRecord = serde_json::from_slice(&json).unwrap();

        assert_eq!(decoded.node_id, "abc123");
        assert_eq!(decoded.relay_url.unwrap(), "https://relay.example.com");
        assert_eq!(decoded.direct_addrs, vec!["192.168.1.1:8080"]);
    }

    #[test]
    fn test_encrypt_decrypt_record_full() {
        let key = derive_encryption_key("test-topic");
        let record = SchellingRecord {
            node_id: "peer123".to_string(),
            relay_url: Some("https://relay.n0.computer".to_string()),
            direct_addrs: vec![
                "203.0.113.1:4433".to_string(),
                "198.51.100.2:4433".to_string(),
            ],
        };

        let plaintext = serde_json::to_vec(&record).unwrap();
        let encrypted = encrypt_record(&plaintext, &key).unwrap();

        // Verify it fits in BEP44 (1000 byte limit)
        assert!(encrypted.len() < 1000, "encrypted record too large: {} bytes", encrypted.len());

        let decrypted = decrypt_record(&encrypted, &key).unwrap();
        let decoded: SchellingRecord = serde_json::from_slice(&decrypted).unwrap();

        assert_eq!(decoded.node_id, "peer123");
        assert_eq!(decoded.relay_url.unwrap(), "https://relay.n0.computer");
        assert_eq!(decoded.direct_addrs.len(), 2);
    }

    #[test]
    fn test_topic_hash_deterministic() {
        let h1 = topic_hash("cats");
        let h2 = topic_hash("cats");

        assert_eq!(h1, h2);
    }

    #[test]
    fn test_topic_hash_different_topics() {
        let h1 = topic_hash("cats");
        let h2 = topic_hash("dogs");

        assert_ne!(h1, h2);
    }

    #[test]
    fn test_signing_and_encryption_keys_independent() {
        // The signing key seed comes from topic_hash[..32] with minute-based HKDF
        // The encryption key comes from topic_hash[32..64] with static HKDF info
        // They should be independent
        let hash = topic_hash("test");

        let signing_ikm = &hash[..32];
        let encrypt_ikm = &hash[32..64];

        assert_ne!(signing_ikm, encrypt_ikm);
    }
}
