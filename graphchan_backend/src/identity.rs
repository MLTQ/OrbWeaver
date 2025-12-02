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
    pub friendcode: String,
    pub gpg_created: bool,
    pub iroh_key_created: bool,
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
    pub addresses: Vec<String>,
}

pub fn ensure_local_identity(paths: &GraphchanPaths) -> Result<IdentitySummary> {
    let (gpg_fingerprint, gpg_created) = ensure_gpg_identity(paths)?;
    let (iroh_peer_id, iroh_key_created) = ensure_iroh_identity(paths)?;
    let friendcode = encode_friendcode(&iroh_peer_id, &gpg_fingerprint)?;

    Ok(IdentitySummary {
        gpg_fingerprint,
        iroh_peer_id,
        friendcode,
        gpg_created,
        iroh_key_created,
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

pub fn encode_friendcode(peer_id: &str, gpg_fingerprint: &str) -> Result<String> {
    let payload = FriendCodePayload {
        version: 1,
        peer_id: peer_id.to_string(),
        gpg_fingerprint: gpg_fingerprint.to_string(),
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
    fn friendcode_roundtrip() {
        let code = encode_friendcode("peer-123", "FINGERPRINT123").unwrap();
        let payload = decode_friendcode(&code).unwrap();
        assert_eq!(payload.peer_id, "peer-123");
        assert_eq!(payload.gpg_fingerprint, "FINGERPRINT123");
        assert!(payload.addresses.is_empty());
    }
}
