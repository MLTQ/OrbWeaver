use crate::database::models::ThreadRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteThreadRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::ThreadRepository for SqliteThreadRepository<'conn> {
    fn create(&self, record: &ThreadRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO threads (id, title, creator_peer_id, created_at, pinned, thread_hash, visibility, topic_secret, sync_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                record.id,
                record.title,
                record.creator_peer_id,
                record.created_at,
                if record.pinned { 1 } else { 0 },
                record.thread_hash,
                record.visibility,
                record.topic_secret,
                record.sync_status
            ],
        )?;
        Ok(())
    }

    fn upsert(&self, record: &ThreadRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO threads (id, title, creator_peer_id, created_at, pinned, thread_hash, visibility, topic_secret, sync_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                creator_peer_id = excluded.creator_peer_id,
                created_at = excluded.created_at,
                pinned = excluded.pinned,
                thread_hash = excluded.thread_hash,
                visibility = excluded.visibility,
                topic_secret = excluded.topic_secret,
                sync_status = excluded.sync_status
            "#,
            params![
                record.id,
                record.title,
                record.creator_peer_id,
                record.created_at,
                if record.pinned { 1 } else { 0 },
                record.thread_hash,
                record.visibility,
                record.topic_secret,
                record.sync_status
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<ThreadRecord>> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT id, title, creator_peer_id, created_at, pinned, thread_hash,
                       COALESCE(visibility, 'social') as visibility, topic_secret,
                       COALESCE(sync_status, 'downloaded') as sync_status
                FROM threads
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(ThreadRecord {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        creator_peer_id: row.get(2)?,
                        created_at: row.get(3)?,
                        pinned: row.get::<_, i64>(4)? != 0,
                        thread_hash: row.get(5)?,
                        visibility: row.get(6)?,
                        topic_secret: row.get(7)?,
                        sync_status: row.get(8)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn list_recent(&self, limit: usize) -> Result<Vec<ThreadRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, creator_peer_id, created_at, pinned, thread_hash,
                   COALESCE(visibility, 'social') as visibility, topic_secret,
                   COALESCE(sync_status, 'downloaded') as sync_status
            FROM threads
            WHERE deleted = 0 AND ignored = 0
            ORDER BY datetime(created_at) DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ThreadRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                creator_peer_id: row.get(2)?,
                created_at: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
                thread_hash: row.get(5)?,
                visibility: row.get(6)?,
                topic_secret: row.get(7)?,
                sync_status: row.get(8)?,
            })
        })?;

        let mut threads = Vec::new();
        for row in rows {
            threads.push(row?);
        }
        Ok(threads)
    }

    fn set_rebroadcast(&self, thread_id: &str, rebroadcast: bool) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE threads
            SET rebroadcast = ?1
            WHERE id = ?2
            "#,
            params![if rebroadcast { 1 } else { 0 }, thread_id],
        )?;
        Ok(())
    }

    fn should_rebroadcast(&self, thread_id: &str) -> Result<bool> {
        let rebroadcast: i64 = self.conn.query_row(
            r#"
            SELECT rebroadcast
            FROM threads
            WHERE id = ?1
            "#,
            params![thread_id],
            |row| row.get(0),
        )?;
        Ok(rebroadcast != 0)
    }

    fn delete(&self, thread_id: &str) -> Result<()> {
        // Real DELETE - will cascade to posts, post_relationships, files, thread_tickets
        tracing::info!("ThreadRepository::delete: Deleting thread_id={}", thread_id);
        let result = self.conn.execute(
            r#"
            DELETE FROM threads
            WHERE id = ?1
            "#,
            params![thread_id],
        );
        match &result {
            Ok(rows) => tracing::info!("ThreadRepository::delete: Deleted {} rows", rows),
            Err(e) => tracing::error!("ThreadRepository::delete FAILED: {:?}", e),
        }
        result?;
        Ok(())
    }

    fn set_ignored(&self, thread_id: &str, ignored: bool) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE threads
            SET ignored = ?1
            WHERE id = ?2
            "#,
            params![if ignored { 1 } else { 0 }, thread_id],
        )?;
        Ok(())
    }

    fn is_ignored(&self, thread_id: &str) -> Result<bool> {
        let ignored: i64 = self.conn.query_row(
            r#"
            SELECT ignored
            FROM threads
            WHERE id = ?1
            "#,
            params![thread_id],
            |row| row.get(0),
        )?;
        Ok(ignored != 0)
    }
}
