use crate::database::models::RedactedPostRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteRedactedPostRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::RedactedPostRepository for SqliteRedactedPostRepository<'conn> {
    fn create(&self, record: &RedactedPostRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO redacted_posts
            (id, thread_id, author_peer_id, parent_post_ids, known_child_ids, redaction_reason, discovered_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                record.id,
                record.thread_id,
                record.author_peer_id,
                record.parent_post_ids,
                record.known_child_ids,
                record.redaction_reason,
                record.discovered_at
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<RedactedPostRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, thread_id, author_peer_id, parent_post_ids, known_child_ids, redaction_reason, discovered_at
            FROM redacted_posts
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(RedactedPostRecord {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    author_peer_id: row.get(2)?,
                    parent_post_ids: row.get(3)?,
                    known_child_ids: row.get(4)?,
                    redaction_reason: row.get(5)?,
                    discovered_at: row.get(6)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<RedactedPostRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, thread_id, author_peer_id, parent_post_ids, known_child_ids, redaction_reason, discovered_at
            FROM redacted_posts
            WHERE thread_id = ?1
            ORDER BY discovered_at
            "#,
        )?;

        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(RedactedPostRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                author_peer_id: row.get(2)?,
                parent_post_ids: row.get(3)?,
                known_child_ids: row.get(4)?,
                redaction_reason: row.get(5)?,
                discovered_at: row.get(6)?,
            })
        })?;

        let mut posts = Vec::new();
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }
}
