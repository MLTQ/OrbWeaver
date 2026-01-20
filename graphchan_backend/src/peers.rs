use crate::database::models::PeerRecord;
use crate::database::repositories::{PeerIpRepository, PeerRepository};
use crate::database::Database;
use crate::identity::{decode_friendcode_auto, encode_short_friendcode, FriendCodePayload};
use crate::utils::now_utc_iso;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::net::IpAddr;

#[derive(Clone)]
pub struct PeerService {
    database: Database,
}

impl PeerService {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    pub fn list_peers(&self) -> Result<Vec<PeerView>> {
        self.database.with_repositories(|repos| {
            let peers = repos.peers().list()?;
            Ok(peers.into_iter().map(PeerView::from_record).collect())
        })
    }

    pub fn get_local_peer(&self) -> Result<Option<PeerView>> {
        let Some((fingerprint, peer_id, friendcode)) = self.database.get_identity()? else {
            return Ok(None);
        };
        let view = self.database.with_repositories(|repos| {
            if let Some(record) = repos.peers().get(&fingerprint)? {
                return Ok(PeerView::from_record(record));
            }
            let record = PeerRecord {
                id: fingerprint.clone(),
                alias: Some("local".into()),
                username: None,
                bio: None,
                friendcode: Some(friendcode.clone()),
                iroh_peer_id: Some(peer_id.clone()),
                gpg_fingerprint: Some(fingerprint.clone()),
                x25519_pubkey: None, // Will be populated from friendcode when available
                last_seen: Some(now_utc_iso()),
                avatar_file_id: None,
                trust_state: "trusted".into(),
                agents: None,
            };
            repos.peers().upsert(&record)?;
            Ok(PeerView::from_record(record))
        })?;
        Ok(Some(view))
    }

    pub fn register_friendcode(&self, friendcode: &str) -> Result<PeerView> {
        let payload = decode_friendcode_auto(friendcode)
            .with_context(|| "failed to decode friendcode".to_string())?;
        let record = payload_to_peer_record(friendcode, &payload);

        // Extract and store IP addresses from multiaddrs
        let ips = extract_ips_from_multiaddrs(&payload.addresses);

        self.database.with_repositories(|repos| {
            repos.peers().upsert(&record)?;

            // Store IP addresses for this peer
            let timestamp = chrono::Utc::now().timestamp();
            for ip in ips {
                if let Err(err) = repos.peer_ips().update(&record.id, &ip.to_string(), timestamp) {
                    tracing::warn!(peer_id = %record.id, ip = %ip, error = ?err, "failed to store peer IP");
                }
            }

            Ok(PeerView::from_record(record))
        })
    }

    pub fn update_profile(&self, peer_id: &str, avatar_file_id: Option<String>, username: Option<String>, bio: Option<String>, agents: Option<Vec<String>>) -> Result<()> {
        self.database.with_repositories(|repos| {
            if let Some(mut record) = repos.peers().get(peer_id)? {
                if avatar_file_id.is_some() {
                    record.avatar_file_id = avatar_file_id;
                }
                if username.is_some() {
                    record.username = username;
                }
                if bio.is_some() {
                    record.bio = bio;
                }
                if let Some(agents_list) = agents {
                    // Serialize agents to JSON
                    record.agents = serde_json::to_string(&agents_list).ok();
                }
                repos.peers().upsert(&record)?;
            } else {
                tracing::warn!("received profile update for unknown peer {}", peer_id);
            }
            Ok(())
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerView {
    pub id: String,
    pub alias: Option<String>,
    pub username: Option<String>,
    pub bio: Option<String>,
    pub friendcode: Option<String>,
    pub short_friendcode: Option<String>,
    pub iroh_peer_id: Option<String>,
    pub gpg_fingerprint: Option<String>,
    pub x25519_pubkey: Option<String>,
    pub last_seen: Option<String>,
    pub avatar_file_id: Option<String>,
    pub trust_state: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agents: Option<Vec<String>>,
}

impl PeerView {
    pub fn from_record(record: PeerRecord) -> Self {
        // Generate short friend code if we have both iroh_peer_id and gpg_fingerprint
        let short_friendcode = match (&record.iroh_peer_id, &record.gpg_fingerprint) {
            (Some(peer_id), Some(fingerprint)) => {
                Some(encode_short_friendcode(peer_id, fingerprint))
            }
            _ => None,
        };

        // Parse agents JSON
        let agents = record.agents.as_ref().and_then(|json_str| {
            serde_json::from_str::<Vec<String>>(json_str).ok()
        });

        Self {
            id: record.id,
            alias: record.alias,
            username: record.username,
            bio: record.bio,
            friendcode: record.friendcode,
            short_friendcode,
            iroh_peer_id: record.iroh_peer_id,
            gpg_fingerprint: record.gpg_fingerprint,
            x25519_pubkey: record.x25519_pubkey,
            last_seen: record.last_seen,
            avatar_file_id: record.avatar_file_id,
            trust_state: record.trust_state,
            agents,
        }
    }
}

fn payload_to_peer_record(friendcode: &str, payload: &FriendCodePayload) -> PeerRecord {
    PeerRecord {
        id: payload.gpg_fingerprint.clone(),
        alias: None,
        username: None,
        bio: None,
        friendcode: Some(friendcode.to_string()),
        iroh_peer_id: Some(payload.peer_id.clone()),
        gpg_fingerprint: Some(payload.gpg_fingerprint.clone()),
        x25519_pubkey: payload.x25519_pubkey.clone(),
        last_seen: Some(now_utc_iso()),
        avatar_file_id: None,
        trust_state: "unknown".into(),
        agents: None,
    }
}

/// Extract IP addresses from multiaddr strings
///
/// Multiaddr format examples:
/// - /ip4/192.168.1.1/udp/8080
/// - /ip6/2001:db8::1/tcp/443
/// - /ip4/10.0.0.1/tcp/9090/p2p/12D3KooW...
pub fn extract_ips_from_multiaddrs(addrs: &[String]) -> Vec<IpAddr> {
    let mut ips = Vec::new();

    for addr in addrs {
        // Split by '/' and find ip4/ip6 components
        let parts: Vec<&str> = addr.split('/').collect();

        for i in 0..parts.len() {
            if i + 1 < parts.len() {
                match parts[i] {
                    "ip4" | "ip6" => {
                        if let Ok(ip) = parts[i + 1].parse::<IpAddr>() {
                            ips.push(ip);
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    ips
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::Database;
    use crate::identity::encode_friendcode;
    use rusqlite::Connection;

    fn setup_service() -> PeerService {
        let conn = Connection::open_in_memory().expect("memory db");
        let db = Database::from_connection(conn, true);
        db.ensure_migrations().expect("migrations");
        PeerService::new(db)
    }

    #[test]
    fn registers_peer_from_friendcode() {
        let service = setup_service();
        let friendcode = encode_friendcode("peer-xyz", "FPRINTXYZ", None).unwrap();
        let view = service.register_friendcode(&friendcode).unwrap();
        assert_eq!(view.gpg_fingerprint.as_deref(), Some("FPRINTXYZ"));
    }

    #[test]
    fn extract_ips_from_multiaddrs_works() {
        let addrs = vec![
            "/ip4/192.168.1.1/udp/8080".to_string(),
            "/ip6/2001:db8::1/tcp/443".to_string(),
            "/ip4/10.0.0.5/tcp/9090/p2p/12D3KooW".to_string(),
            "/invalid/address".to_string(),
        ];

        let ips = extract_ips_from_multiaddrs(&addrs);

        assert_eq!(ips.len(), 3);
        assert!(ips.contains(&"192.168.1.1".parse::<IpAddr>().unwrap()));
        assert!(ips.contains(&"2001:db8::1".parse::<IpAddr>().unwrap()));
        assert!(ips.contains(&"10.0.0.5".parse::<IpAddr>().unwrap()));
    }
}
