use crate::database::models::PeerRecord;
use crate::database::repositories::PeerRepository;
use crate::database::Database;
use crate::identity::{decode_friendcode, FriendCodePayload};
use crate::utils::now_utc_iso;
use anyhow::{Context, Result};
use serde::Serialize;

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
                friendcode: Some(friendcode.clone()),
                iroh_peer_id: Some(peer_id.clone()),
                gpg_fingerprint: Some(fingerprint.clone()),
                last_seen: Some(now_utc_iso()),
                trust_state: "trusted".into(),
            };
            repos.peers().upsert(&record)?;
            Ok(PeerView::from_record(record))
        })?;
        Ok(Some(view))
    }

    pub fn register_friendcode(&self, friendcode: &str) -> Result<PeerView> {
        let payload = decode_friendcode(friendcode)
            .with_context(|| "failed to decode friendcode".to_string())?;
        let record = payload_to_peer_record(friendcode, &payload);
        self.database.with_repositories(|repos| {
            repos.peers().upsert(&record)?;
            Ok(PeerView::from_record(record))
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PeerView {
    pub id: String,
    pub alias: Option<String>,
    pub friendcode: Option<String>,
    pub iroh_peer_id: Option<String>,
    pub gpg_fingerprint: Option<String>,
    pub last_seen: Option<String>,
    pub trust_state: String,
}

impl PeerView {
    fn from_record(record: PeerRecord) -> Self {
        Self {
            id: record.id,
            alias: record.alias,
            friendcode: record.friendcode,
            iroh_peer_id: record.iroh_peer_id,
            gpg_fingerprint: record.gpg_fingerprint,
            last_seen: record.last_seen,
            trust_state: record.trust_state,
        }
    }
}

fn payload_to_peer_record(friendcode: &str, payload: &FriendCodePayload) -> PeerRecord {
    PeerRecord {
        id: payload.gpg_fingerprint.clone(),
        alias: None,
        friendcode: Some(friendcode.to_string()),
        iroh_peer_id: Some(payload.peer_id.clone()),
        gpg_fingerprint: Some(payload.gpg_fingerprint.clone()),
        last_seen: Some(now_utc_iso()),
        trust_state: "unknown".into(),
    }
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
        let friendcode = encode_friendcode("peer-xyz", "FPRINTXYZ").unwrap();
        let view = service.register_friendcode(&friendcode).unwrap();
        assert_eq!(view.gpg_fingerprint.as_deref(), Some("FPRINTXYZ"));
    }
}
