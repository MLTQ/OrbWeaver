use crate::database::models::PeerRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqlitePeerRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::PeerRepository for SqlitePeerRepository<'conn> {
    fn upsert(&self, record: &PeerRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO peers (id, alias, friendcode, iroh_peer_id, gpg_fingerprint, x25519_pubkey, last_seen, trust_state, avatar_file_id, username, bio, agents)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
            ON CONFLICT(id) DO UPDATE SET
                alias = excluded.alias,
                friendcode = excluded.friendcode,
                iroh_peer_id = excluded.iroh_peer_id,
                gpg_fingerprint = excluded.gpg_fingerprint,
                x25519_pubkey = excluded.x25519_pubkey,
                last_seen = excluded.last_seen,
                trust_state = excluded.trust_state,
                avatar_file_id = excluded.avatar_file_id,
                username = excluded.username,
                bio = excluded.bio,
                agents = excluded.agents
            "#,
            params![
                record.id,
                record.alias,
                record.friendcode,
                record.iroh_peer_id,
                record.gpg_fingerprint,
                record.x25519_pubkey,
                record.last_seen,
                record.trust_state,
                record.avatar_file_id,
                record.username,
                record.bio,
                record.agents
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<PeerRecord>> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT id, alias, friendcode, iroh_peer_id, gpg_fingerprint, x25519_pubkey, last_seen, trust_state, avatar_file_id, username, bio, agents
                FROM peers
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(PeerRecord {
                        id: row.get(0)?,
                        alias: row.get(1)?,
                        friendcode: row.get(2)?,
                        iroh_peer_id: row.get(3)?,
                        gpg_fingerprint: row.get(4)?,
                        x25519_pubkey: row.get(5)?,
                        last_seen: row.get(6)?,
                        trust_state: row.get(7)?,
                        avatar_file_id: row.get(8)?,
                        username: row.get(9)?,
                        bio: row.get(10)?,
                        agents: row.get(11)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn list(&self) -> Result<Vec<PeerRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, alias, friendcode, iroh_peer_id, gpg_fingerprint, x25519_pubkey, last_seen, trust_state, avatar_file_id, username, bio, agents
            FROM peers
            ORDER BY datetime(COALESCE(last_seen, '1970-01-01T00:00:00Z')) DESC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PeerRecord {
                id: row.get(0)?,
                alias: row.get(1)?,
                friendcode: row.get(2)?,
                iroh_peer_id: row.get(3)?,
                gpg_fingerprint: row.get(4)?,
                x25519_pubkey: row.get(5)?,
                last_seen: row.get(6)?,
                trust_state: row.get(7)?,
                avatar_file_id: row.get(8)?,
                username: row.get(9)?,
                bio: row.get(10)?,
                agents: row.get(11)?,
            })
        })?;
        let mut peers = Vec::new();
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
    }

    fn delete(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM peers WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}
