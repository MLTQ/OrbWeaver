use crate::config::GraphchanPaths;
use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::STANDARD as BASE64, Engine as _};
use iroh_base::SecretKey;
use rand::rng;
use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use uuid::Uuid;

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

    run_gpg_command(
        homedir,
        ["--quick-generate-key", &user_id, "ed25519", "sign", "0"],
    )
    .context("failed to generate primary GPG key")?;

    let fingerprint = read_gpg_fingerprint(homedir, &user_id)?;

    run_gpg_command(
        homedir,
        ["--quick-add-key", &fingerprint, "cv25519", "encrypt", "0"],
    )
    .context("failed to add encryption subkey")?;

    export_gpg_key_material(
        homedir,
        &fingerprint,
        &paths.gpg_public_key,
        &paths.gpg_private_key,
    )?;

    tighten_permissions(&paths.gpg_private_key.parent().unwrap_or(homedir))?;
    tighten_permissions(&paths.gpg_private_key)?;
    tighten_permissions(&paths.gpg_public_key)?;

    Ok(fingerprint)
}

fn run_gpg_command<'a, I>(homedir: &Path, extra_args: I) -> Result<()>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut cmd = Command::new("gpg");
    cmd.arg("--homedir")
        .arg(homedir)
        .arg("--batch")
        .arg("--yes")
        .arg("--pinentry-mode")
        .arg("loopback")
        .arg("--passphrase")
        .arg("")
        .args(extra_args);
    let status = cmd
        .status()
        .with_context(|| format!("failed to invoke gpg (homedir: {})", homedir.display()))?;
    if !status.success() {
        return Err(anyhow!("gpg command exited with status {status}").context("gpg failure"));
    }
    Ok(())
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

fn read_gpg_fingerprint(homedir: &Path, user_id: &str) -> Result<String> {
    let output = Command::new("gpg")
        .arg("--homedir")
        .arg(homedir)
        .arg("--batch")
        .arg("--with-colons")
        .arg("--list-secret-keys")
        .arg(user_id)
        .output()
        .with_context(|| format!("failed to list gpg keys for {user_id}"))?;

    if !output.status.success() {
        return Err(anyhow!(
            "gpg --list-secret-keys failed with status {}",
            output.status
        ));
    }

    let stdout = String::from_utf8(output.stdout)?;
    for line in stdout.lines() {
        if let Some(rest) = line.strip_prefix("fpr:") {
            let parts: Vec<&str> = rest.split(':').collect();
            if parts.len() >= 9 {
                let fingerprint = parts[8].trim().to_string();
                if !fingerprint.is_empty() {
                    return Ok(fingerprint);
                }
            }
        }
    }

    Err(anyhow!(
        "failed to parse GPG fingerprint for newly created key"
    ))
}

fn export_gpg_key_material(
    homedir: &Path,
    fingerprint: &str,
    public_dest: &Path,
    private_dest: &Path,
) -> Result<()> {
    let public = Command::new("gpg")
        .arg("--homedir")
        .arg(homedir)
        .arg("--batch")
        .arg("--yes")
        .arg("--armor")
        .arg("--export")
        .arg(fingerprint)
        .output()
        .with_context(|| format!("failed to export public key for {fingerprint}"))?;
    if !public.status.success() {
        return Err(anyhow!("gpg --export failed with status {}", public.status));
    }
    fs::write(public_dest, public.stdout)?;

    let secret = Command::new("gpg")
        .arg("--homedir")
        .arg(homedir)
        .arg("--batch")
        .arg("--yes")
        .arg("--armor")
        .arg("--export-secret-keys")
        .arg(fingerprint)
        .output()
        .with_context(|| format!("failed to export secret key for {fingerprint}"))?;
    if !secret.status.success() {
        return Err(anyhow!(
            "gpg --export-secret-keys failed with status {}",
            secret.status
        ));
    }
    fs::write(private_dest, secret.stdout)?;
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
