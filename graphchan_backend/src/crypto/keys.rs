use crate::config::GraphchanPaths;
use anyhow::{Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use x25519_dalek::{PublicKey, StaticSecret};

const ENCRYPTION_KEY_FILE: &str = "encryption.key";

#[derive(Clone)]
pub struct X25519Identity {
    pub secret: StaticSecret,
    pub public: PublicKey,
}

impl std::fmt::Debug for X25519Identity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("X25519Identity")
            .field("public", &BASE64.encode(self.public.as_bytes()))
            .field("secret", &"<redacted>")
            .finish()
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct StoredX25519Identity {
    version: u8,
    public_key_b64: String,
    secret_key_b64: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WrappedKey {
    pub ciphertext: Vec<u8>,
    pub nonce: [u8; 24],
}

/// Ensures the X25519 encryption keypair exists, creating it if necessary.
/// Returns (public_key_base64, was_created).
pub fn ensure_x25519_identity(paths: &GraphchanPaths) -> Result<(String, bool)> {
    let key_path = paths.keys_dir.join(ENCRYPTION_KEY_FILE);

    if key_path.exists() {
        let stored = load_stored_identity(&key_path)?;
        return Ok((stored.public_key_b64, false));
    }

    // Generate new keypair using rand for compatibility
    let mut secret_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut secret_bytes);
    let secret = StaticSecret::from(secret_bytes);
    let public = PublicKey::from(&secret);

    let stored = StoredX25519Identity {
        version: 1,
        public_key_b64: BASE64.encode(public.as_bytes()),
        secret_key_b64: BASE64.encode(secret.to_bytes()),
    };

    let json = serde_json::to_string_pretty(&stored)?;
    fs::write(&key_path, json)?;

    // Tighten permissions on Unix
    #[cfg(unix)]
    {
        let metadata = fs::metadata(&key_path)?;
        let mut permissions = metadata.permissions();
        permissions.set_mode(0o600);
        fs::set_permissions(&key_path, permissions)?;
    }

    tracing::info!("generated new X25519 encryption keypair");
    Ok((stored.public_key_b64, true))
}

/// Loads the X25519 secret key from disk.
pub fn load_x25519_secret(paths: &GraphchanPaths) -> Result<X25519Identity> {
    let key_path = paths.keys_dir.join(ENCRYPTION_KEY_FILE);
    let stored = load_stored_identity(&key_path)?;

    let secret_bytes = BASE64
        .decode(&stored.secret_key_b64)
        .context("failed to decode secret key base64")?;

    let secret_array: [u8; 32] = secret_bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("secret key must be 32 bytes"))?;

    let secret = StaticSecret::from(secret_array);
    let public = PublicKey::from(&secret);

    Ok(X25519Identity { secret, public })
}

fn load_stored_identity(path: &std::path::Path) -> Result<StoredX25519Identity> {
    let json = fs::read_to_string(path)
        .with_context(|| format!("failed to read X25519 key from {}", path.display()))?;

    serde_json::from_str(&json)
        .context("failed to deserialize X25519 identity")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_x25519_identity_creation() {
        let temp = TempDir::new().unwrap();
        let paths = GraphchanPaths {
            keys_dir: temp.path().to_path_buf(),
            ..Default::default()
        };

        let (pubkey1, created1) = ensure_x25519_identity(&paths).unwrap();
        assert!(created1);
        assert!(!pubkey1.is_empty());

        // Second call should not create a new key
        let (pubkey2, created2) = ensure_x25519_identity(&paths).unwrap();
        assert!(!created2);
        assert_eq!(pubkey1, pubkey2);
    }

    #[test]
    fn test_load_x25519_secret() {
        let temp = TempDir::new().unwrap();
        let paths = GraphchanPaths {
            keys_dir: temp.path().to_path_buf(),
            ..Default::default()
        };

        ensure_x25519_identity(&paths).unwrap();
        let identity = load_x25519_secret(&paths).unwrap();

        // Verify public key matches
        let expected_public = PublicKey::from(&identity.secret);
        assert_eq!(identity.public.as_bytes(), expected_public.as_bytes());
    }
}
