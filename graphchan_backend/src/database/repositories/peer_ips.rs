use crate::database::models::PeerIpRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqlitePeerIpRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::PeerIpRepository for SqlitePeerIpRepository<'conn> {
    fn update(&self, peer_id: &str, ip_address: &str, last_seen: i64) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO peer_ips (peer_id, ip_address, last_seen)
            VALUES (?1, ?2, ?3)
            ON CONFLICT(peer_id, ip_address) DO UPDATE SET
                last_seen = excluded.last_seen
            "#,
            params![peer_id, ip_address, last_seen],
        )?;
        Ok(())
    }

    fn get(&self, peer_id: &str) -> Result<Option<PeerIpRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT peer_id, ip_address, last_seen
            FROM peer_ips
            WHERE peer_id = ?1
            ORDER BY last_seen DESC
            LIMIT 1
            "#,
            params![peer_id],
            |row| {
                Ok(PeerIpRecord {
                    peer_id: row.get(0)?,
                    ip_address: row.get(1)?,
                    last_seen: row.get(2)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn get_by_ip(&self, ip_address: &str) -> Result<Vec<PeerIpRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT peer_id, ip_address, last_seen
            FROM peer_ips
            WHERE ip_address = ?1
            ORDER BY last_seen DESC
            "#,
        )?;

        let rows = stmt.query_map(params![ip_address], |row| {
            Ok(PeerIpRecord {
                peer_id: row.get(0)?,
                ip_address: row.get(1)?,
                last_seen: row.get(2)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    fn get_ips(&self, peer_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT ip_address
            FROM peer_ips
            WHERE peer_id = ?1
            ORDER BY last_seen DESC
            "#,
        )?;

        let rows = stmt.query_map(params![peer_id], |row| {
            row.get::<_, String>(0)
        })?;

        let mut ips = Vec::new();
        for row in rows {
            ips.push(row?);
        }
        Ok(ips)
    }

    fn list_all(&self) -> Result<Vec<PeerIpRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT peer_id, ip_address, last_seen
            FROM peer_ips
            ORDER BY last_seen DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(PeerIpRecord {
                peer_id: row.get(0)?,
                ip_address: row.get(1)?,
                last_seen: row.get(2)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }
}
