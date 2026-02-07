use crate::database::models::PostRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqlitePostRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::PostRepository for SqlitePostRepository<'conn> {
    fn create(&self, record: &PostRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO posts (id, thread_id, author_peer_id, author_friendcode, body, created_at, updated_at, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                record.id,
                record.thread_id,
                record.author_peer_id,
                record.author_friendcode,
                record.body,
                record.created_at,
                record.updated_at,
                record.metadata
            ],
        )?;
        Ok(())
    }

    fn upsert(&self, record: &PostRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO posts (id, thread_id, author_peer_id, author_friendcode, body, created_at, updated_at, metadata)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
                thread_id = excluded.thread_id,
                author_peer_id = excluded.author_peer_id,
                author_friendcode = excluded.author_friendcode,
                body = excluded.body,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at,
                metadata = excluded.metadata
            "#,
            params![
                record.id,
                record.thread_id,
                record.author_peer_id,
                record.author_friendcode,
                record.body,
                record.created_at,
                record.updated_at,
                record.metadata
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<PostRecord>> {
        Ok(self
            .conn
            .query_row(
                r#"
                SELECT id, thread_id, author_peer_id, author_friendcode, body, created_at, updated_at, metadata
                FROM posts
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(PostRecord {
                        id: row.get(0)?,
                        thread_id: row.get(1)?,
                        author_peer_id: row.get(2)?,
                        author_friendcode: row.get(3)?,
                        body: row.get(4)?,
                        created_at: row.get(5)?,
                        updated_at: row.get(6)?,
                        metadata: row.get(7)?,
                    })
                },
            )
            .optional()?)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<PostRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, thread_id, author_peer_id, author_friendcode, body, created_at, updated_at, metadata
            FROM posts
            WHERE thread_id = ?1
            ORDER BY datetime(created_at) ASC
            "#,
        )?;
        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(PostRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                author_peer_id: row.get(2)?,
                author_friendcode: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                metadata: row.get(7)?,
            })
        })?;
        let mut posts = Vec::new();
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }

    fn list_recent(&self, limit: usize) -> Result<Vec<PostRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT p.id, p.thread_id, p.author_peer_id, p.author_friendcode, p.body, p.created_at, p.updated_at, p.metadata
            FROM posts p
            INNER JOIN threads t ON p.thread_id = t.id
            WHERE t.sync_status = 'downloaded'
            ORDER BY datetime(p.created_at) DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit], |row| {
            Ok(PostRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                author_peer_id: row.get(2)?,
                author_friendcode: row.get(3)?,
                body: row.get(4)?,
                created_at: row.get(5)?,
                updated_at: row.get(6)?,
                metadata: row.get(7)?,
            })
        })?;
        let mut posts = Vec::new();
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }

    fn add_relationships(&self, child_id: &str, parent_ids: &[String]) -> Result<()> {
        if parent_ids.is_empty() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR IGNORE INTO post_relationships (parent_id, child_id)
                VALUES (?1, ?2)
                "#,
            )?;
            for parent in parent_ids {
                stmt.execute(params![parent, child_id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn parents_of(&self, child_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT parent_id
            FROM post_relationships
            WHERE child_id = ?1
            ORDER BY parent_id ASC
            "#,
        )?;
        let rows = stmt.query_map(params![child_id], |row| row.get::<_, String>(0))?;
        let mut parents = Vec::new();
        for row in rows {
            parents.push(row?);
        }
        Ok(parents)
    }

    fn has_children(&self, post_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM post_relationships
            WHERE parent_id = ?1
            "#,
            params![post_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}
