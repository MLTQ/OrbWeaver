use crate::database::models::{BlocklistEntryRecord, BlocklistSubscriptionRecord};
use anyhow::Result;
use rusqlite::{params, Connection};

pub(super) struct SqliteBlocklistRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::BlocklistRepository for SqliteBlocklistRepository<'conn> {
    fn subscribe(&self, record: &BlocklistSubscriptionRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO blocklist_subscriptions
            (id, maintainer_peer_id, name, description, auto_apply, last_synced_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                record.id,
                record.maintainer_peer_id,
                record.name,
                record.description,
                if record.auto_apply { 1 } else { 0 },
                record.last_synced_at
            ],
        )?;
        Ok(())
    }

    fn unsubscribe(&self, blocklist_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM blocklist_subscriptions
            WHERE id = ?1
            "#,
            params![blocklist_id],
        )?;
        Ok(())
    }

    fn list_subscriptions(&self) -> Result<Vec<BlocklistSubscriptionRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, maintainer_peer_id, name, description, auto_apply, last_synced_at
            FROM blocklist_subscriptions
            ORDER BY name
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BlocklistSubscriptionRecord {
                id: row.get(0)?,
                maintainer_peer_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                auto_apply: row.get::<_, i64>(4)? != 0,
                last_synced_at: row.get(5)?,
            })
        })?;

        let mut lists = Vec::new();
        for row in rows {
            lists.push(row?);
        }
        Ok(lists)
    }

    fn add_entry(&self, entry: &BlocklistEntryRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO blocklist_entries (blocklist_id, peer_id, reason, added_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![entry.blocklist_id, entry.peer_id, entry.reason, entry.added_at],
        )?;
        Ok(())
    }

    fn remove_entry(&self, blocklist_id: &str, peer_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM blocklist_entries
            WHERE blocklist_id = ?1 AND peer_id = ?2
            "#,
            params![blocklist_id, peer_id],
        )?;
        Ok(())
    }

    fn list_entries(&self, blocklist_id: &str) -> Result<Vec<BlocklistEntryRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT blocklist_id, peer_id, reason, added_at
            FROM blocklist_entries
            WHERE blocklist_id = ?1
            ORDER BY added_at DESC
            "#,
        )?;

        let rows = stmt.query_map(params![blocklist_id], |row| {
            Ok(BlocklistEntryRecord {
                blocklist_id: row.get(0)?,
                peer_id: row.get(1)?,
                reason: row.get(2)?,
                added_at: row.get(3)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    fn is_in_any_blocklist(&self, peer_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM blocklist_entries e
            JOIN blocklist_subscriptions s ON e.blocklist_id = s.id
            WHERE e.peer_id = ?1 AND s.auto_apply = 1
            "#,
            params![peer_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}
