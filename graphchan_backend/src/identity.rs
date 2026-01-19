use crate::config::GraphchanPaths;
use anyhow::{anyhow, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use iroh_base::SecretKey;
use rand::rng;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use uuid::Uuid;

use sequoia_openpgp as openpgp;
use openpgp::cert::CertBuilder;
use openpgp::serialize::Serialize as _; // Import trait anonymously to avoid conflict with serde::Serialize

const FINGERPRINT_FILE: &str = "fingerprint.txt";

#[derive(Debug, Clone)]
pub struct IdentitySummary {
    pub gpg_fingerprint: String,
    pub iroh_peer_id: String,
    pub x25519_pubkey: String,
    pub friendcode: String,
    pub short_friendcode: String,
    pub gpg_created: bool,
    pub iroh_key_created: bool,
    pub x25519_created: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredIrohIdentity {
    version: u8,
    peer_id: String,
    secret_key_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FriendCodePayload {
    pub version: u8,
    pub peer_id: String,
    pub gpg_fingerprint: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub x25519_pubkey: Option<String>,
    pub addresses: Vec<String>,
}

pub fn ensure_local_identity(paths: &GraphchanPaths) -> Result<IdentitySummary> {
    let (gpg_fingerprint, gpg_created) = ensure_gpg_identity(paths)?;
    let (iroh_peer_id, iroh_key_created) = ensure_iroh_identity(paths)?;
    let (x25519_pubkey, x25519_created) = crate::crypto::ensure_x25519_identity(paths)?;
    let friendcode = encode_friendcode(&iroh_peer_id, &gpg_fingerprint, Some(&x25519_pubkey))?;
    let short_friendcode = encode_short_friendcode(&iroh_peer_id, &gpg_fingerprint);

    Ok(IdentitySummary {
        gpg_fingerprint,
        iroh_peer_id,
        x25519_pubkey,
        friendcode,
        short_friendcode,
        gpg_created,
        iroh_key_created,
        x25519_created,
    })
}

fn ensure_gpg_identity(paths: &GraphchanPaths) -> Result<(String, bool)> {
    let fingerprint_path = paths.gpg_dir.join(FINGERPRINT_FILE);
    if fingerprint_path.exists() {
        let fingerprint = fs::read_to_string(&fingerprint_path)?.trim().to_string();
        if !fingerprint.is_empty() {
            return Ok((fingerprint, false));
        }
    }

    let fingerprint = generate_gpg_identity(paths)?;
    fs::write(&fingerprint_path, &fingerprint)?;
    Ok((fingerprint, true))
}

fn generate_gpg_identity(paths: &GraphchanPaths) -> Result<String> {
    fs::create_dir_all(&paths.gpg_dir)?;
    tighten_permissions(&paths.gpg_dir)?;
    let homedir = &paths.gpg_dir;
    let node_id = Uuid::new_v4();
    let uid = format!("Graphchan Node {node_id}");
    let email = format!("node-{node_id}@graphchan.local");
    let user_id = format!("{uid} <{email}>");

    // Generate a new Cert (Primary Key + Subkeys)
    // We want Ed25519 for signing (primary) and CV25519 for encryption (subkey)
    // set_cipher_suite(Cv25519) creates an Ed25519 primary key (Sign, Certify) and a Cv25519 subkey (Encrypt)
    let (cert, _revocation) = CertBuilder::new()
        .add_userid(user_id.as_str())
        .set_cipher_suite(openpgp::cert::CipherSuite::Cv25519)
        .generate()?;

    let fingerprint = cert.fingerprint().to_string();

    // Export Public Key
    cert.armored().serialize(&mut fs::File::create(&paths.gpg_public_key)?)?;

    // Export Private Key
    cert.as_tsk().armored().serialize(&mut fs::File::create(&paths.gpg_private_key)?)?;

    tighten_permissions(&paths.gpg_private_key.parent().unwrap_or(homedir))?;
    tighten_permissions(&paths.gpg_private_key)?;
    tighten_permissions(&paths.gpg_public_key)?;

    Ok(fingerprint)
}

fn tighten_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let perms = if path.is_dir() {
            fs::Permissions::from_mode(0o700)
        } else {
            fs::Permissions::from_mode(0o600)
        };
        if let Err(err) = fs::set_permissions(path, perms) {
            tracing::warn!(path = %path.display(), error = ?err, "failed to tighten permissions");
        }
    }
    Ok(())
}

fn ensure_iroh_identity(paths: &GraphchanPaths) -> Result<(String, bool)> {
    if paths.iroh_key_path.exists() {
        if let Ok((peer_id, _secret)) = load_iroh_identity(&paths.iroh_key_path) {
            return Ok((peer_id, false));
        }
    }

    let mut rng = rng();
    let secret = SecretKey::generate(&mut rng);
    let public = secret.public();
    let peer_id = public.to_string();
    let encoded = BASE64.encode(secret.to_bytes());
    let stored = StoredIrohIdentity {
        version: 1,
        peer_id: peer_id.clone(),
        secret_key_b64: encoded,
    };
    let json = serde_json::to_string_pretty(&stored)?;
    fs::write(&paths.iroh_key_path, json)?;
    Ok((peer_id, true))
}

fn load_iroh_identity(path: &Path) -> Result<(String, SecretKey)> {
    let contents = fs::read_to_string(path)?;
    let stored: StoredIrohIdentity = serde_json::from_str(&contents)?;
    let key_bytes = BASE64.decode(stored.secret_key_b64.as_bytes())?;
    let secret = SecretKey::try_from(&key_bytes[..])
        .map_err(|err| anyhow!("failed to deserialize Iroh secret key: {err}"))?;
    Ok((stored.peer_id, secret))
}

pub fn encode_friendcode(peer_id: &str, gpg_fingerprint: &str, x25519_pubkey: Option<&str>) -> Result<String> {
    let version = if x25519_pubkey.is_some() { 2 } else { 1 };
    let payload = FriendCodePayload {
        version,
        peer_id: peer_id.to_string(),
        gpg_fingerprint: gpg_fingerprint.to_string(),
        x25519_pubkey: x25519_pubkey.map(|s| s.to_string()),
        addresses: advertised_addresses(),
    };
    let json = serde_json::to_vec(&payload)?;
    Ok(BASE64.encode(json))
}

pub fn decode_friendcode(friendcode: &str) -> Result<FriendCodePayload> {
    let bytes = BASE64.decode(friendcode.as_bytes())?;
    let payload: FriendCodePayload = serde_json::from_slice(&bytes)?;
    Ok(payload)
}

/// Generate a short friend code containing just the peer ID and GPG fingerprint
/// Format: "graphchan:{peer_id}:{gpg_fingerprint}"
/// Example: "graphchan:abc123def456...xyz789:ABCD1234EFGH5678..."
///
/// This format is much shorter (~120 chars) than the legacy base64 format (~400+ chars)
/// and relies on DHT discovery to resolve the peer's addresses automatically.
pub fn encode_short_friendcode(peer_id: &str, gpg_fingerprint: &str) -> String {
    format!("graphchan:{}:{}", peer_id, gpg_fingerprint)
}

/// Decode a short friend code
/// Returns (peer_id, gpg_fingerprint)
pub fn decode_short_friendcode(friendcode: &str) -> Result<(String, String)> {
    let friendcode = friendcode.trim();

    // Check prefix
    if !friendcode.starts_with("graphchan:") {
        anyhow::bail!("Invalid friend code format: missing 'graphchan:' prefix");
    }

    // Strip prefix
    let data = &friendcode[10..]; // "graphchan:".len() = 10

    // Split on ':'
    let parts: Vec<&str> = data.split(':').collect();
    if parts.len() != 2 {
        anyhow::bail!("Invalid friend code format: expected peer_id:gpg_fingerprint");
    }

    Ok((parts[0].to_string(), parts[1].to_string()))
}

/// Try to decode either short or legacy friend code formats
/// This provides backward compatibility while supporting the new shorter format
pub fn decode_friendcode_auto(friendcode: &str) -> Result<FriendCodePayload> {
    let friendcode = friendcode.trim();

    // Try short format first (starts with "graphchan:")
    if friendcode.starts_with("graphchan:") {
        let (peer_id, gpg_fingerprint) = decode_short_friendcode(friendcode)?;
        return Ok(FriendCodePayload {
            version: 3, // New version for short codes
            peer_id,
            gpg_fingerprint,
            x25519_pubkey: None, // Will be negotiated on connection via DH key exchange
            addresses: vec![], // DHT will resolve addresses automatically
        });
    }

    // Fall back to legacy base64 format
    decode_friendcode(friendcode)
}

pub fn load_iroh_secret(paths: &GraphchanPaths) -> Result<SecretKey> {
    let (_, secret) = load_iroh_identity(&paths.iroh_key_path)?;
    Ok(secret)
}

fn advertised_addresses() -> Vec<String> {
    env::var("GRAPHCHAN_PUBLIC_ADDRS")
        .ok()
        .map(|raw| {
            raw.split(',')
                .map(|part| part.trim())
                .filter(|part| !part.is_empty())
                .map(|part| part.to_string())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::{decode_friendcode, encode_friendcode};

    #[test]
    fn friendcode_v1_roundtrip() {
        let code = encode_friendcode("peer-123", "FINGERPRINT123", None).unwrap();
        let payload = decode_friendcode(&code).unwrap();
        assert_eq!(payload.version, 1);
        assert_eq!(payload.peer_id, "peer-123");
        assert_eq!(payload.gpg_fingerprint, "FINGERPRINT123");
        assert!(payload.x25519_pubkey.is_none());
        assert!(payload.addresses.is_empty());
    }

    #[test]
    fn friendcode_v2_roundtrip() {
        let code = encode_friendcode("peer-456", "FINGERPRINT456", Some("x25519pubkey")).unwrap();
        let payload = decode_friendcode(&code).unwrap();
        assert_eq!(payload.version, 2);
        assert_eq!(payload.peer_id, "peer-456");
        assert_eq!(payload.gpg_fingerprint, "FINGERPRINT456");
        assert_eq!(payload.x25519_pubkey.as_deref(), Some("x25519pubkey"));
        assert!(payload.addresses.is_empty());
    }
}
