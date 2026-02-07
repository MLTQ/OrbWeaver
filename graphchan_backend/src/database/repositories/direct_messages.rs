use crate::database::models::DirectMessageRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteDirectMessageRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::DirectMessageRepository for SqliteDirectMessageRepository<'conn> {
    fn create(&self, record: &DirectMessageRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO direct_messages (id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                record.id,
                record.conversation_id,
                record.from_peer_id,
                record.to_peer_id,
                record.encrypted_body,
                record.nonce,
                record.created_at,
                record.read_at
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<DirectMessageRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at
            FROM direct_messages
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(DirectMessageRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    from_peer_id: row.get(2)?,
                    to_peer_id: row.get(3)?,
                    encrypted_body: row.get(4)?,
                    nonce: row.get(5)?,
                    created_at: row.get(6)?,
                    read_at: row.get(7)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list_for_conversation(&self, conversation_id: &str, limit: usize) -> Result<Vec<DirectMessageRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at
            FROM direct_messages
            WHERE conversation_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![conversation_id, limit as i64], |row| {
            Ok(DirectMessageRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                from_peer_id: row.get(2)?,
                to_peer_id: row.get(3)?,
                encrypted_body: row.get(4)?,
                nonce: row.get(5)?,
                created_at: row.get(6)?,
                read_at: row.get(7)?,
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    fn mark_as_read(&self, id: &str, read_at: &str) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE direct_messages
            SET read_at = ?1
            WHERE id = ?2
            "#,
            params![read_at, id],
        )?;
        Ok(())
    }

    fn count_unread(&self, to_peer_id: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM direct_messages
            WHERE to_peer_id = ?1 AND read_at IS NULL
            "#,
            params![to_peer_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}
