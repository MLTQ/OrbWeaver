use crate::database::models::BlockedPeerRecord;
use anyhow::Result;
use rusqlite::{params, Connection};

pub(super) struct SqliteBlockedPeerRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::BlockedPeerRepository for SqliteBlockedPeerRepository<'conn> {
    fn block(&self, record: &BlockedPeerRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO blocked_peers (peer_id, reason, blocked_at)
            VALUES (?1, ?2, ?3)
            "#,
            params![record.peer_id, record.reason, record.blocked_at],
        )?;
        Ok(())
    }

    fn unblock(&self, peer_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM blocked_peers
            WHERE peer_id = ?1
            "#,
            params![peer_id],
        )?;
        Ok(())
    }

    fn is_blocked(&self, peer_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM blocked_peers
            WHERE peer_id = ?1
            "#,
            params![peer_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn list(&self) -> Result<Vec<BlockedPeerRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT peer_id, reason, blocked_at
            FROM blocked_peers
            ORDER BY blocked_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BlockedPeerRecord {
                peer_id: row.get(0)?,
                reason: row.get(1)?,
                blocked_at: row.get(2)?,
            })
        })?;

        let mut peers = Vec::new();
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
    }
}
