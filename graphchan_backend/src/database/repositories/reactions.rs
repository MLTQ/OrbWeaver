use crate::database::models::ReactionRecord;
use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;

pub(super) struct SqliteReactionRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::ReactionRepository for SqliteReactionRepository<'conn> {
    fn add(&self, record: &ReactionRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO reactions (post_id, reactor_peer_id, emoji, signature, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(post_id, reactor_peer_id, emoji) DO UPDATE SET
                signature = excluded.signature,
                created_at = excluded.created_at
            "#,
            params![
                record.post_id,
                record.reactor_peer_id,
                record.emoji,
                record.signature,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    fn remove(&self, post_id: &str, reactor_peer_id: &str, emoji: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM reactions
            WHERE post_id = ?1 AND reactor_peer_id = ?2 AND emoji = ?3
            "#,
            params![post_id, reactor_peer_id, emoji],
        )?;
        Ok(())
    }

    fn list_for_post(&self, post_id: &str) -> Result<Vec<ReactionRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT post_id, reactor_peer_id, emoji, signature, created_at
            FROM reactions
            WHERE post_id = ?1
            ORDER BY created_at ASC
            "#,
        )?;
        let rows = stmt.query_map(params![post_id], |row| {
            Ok(ReactionRecord {
                post_id: row.get(0)?,
                reactor_peer_id: row.get(1)?,
                emoji: row.get(2)?,
                signature: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut reactions = Vec::new();
        for row in rows {
            reactions.push(row?);
        }
        Ok(reactions)
    }

    fn count_for_post(&self, post_id: &str) -> Result<HashMap<String, usize>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT emoji, COUNT(*) as count
            FROM reactions
            WHERE post_id = ?1
            GROUP BY emoji
            "#,
        )?;
        let rows = stmt.query_map(params![post_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;

        let mut counts = HashMap::new();
        for row in rows {
            let (emoji, count) = row?;
            counts.insert(emoji, count);
        }
        Ok(counts)
    }
}
