use crate::database::models::DirectMessageRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteDirectMessageRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::DirectMessageRepository for SqliteDirectMessageRepository<'conn> {
    fn create(&self, record: &DirectMessageRecord) -> Result<()> {
        // ON CONFLICT(id) DO NOTHING: re-receiving the same DM (gossip rebroadcast
        // after restart, when the in-memory dedup cache is empty) is idempotent
        // rather than a constraint violation that drops the message.
        self.conn.execute(
            r#"
            INSERT INTO direct_messages (id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at, decrypt_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO NOTHING
            "#,
            params![
                record.id,
                record.conversation_id,
                record.from_peer_id,
                record.to_peer_id,
                record.encrypted_body,
                record.nonce,
                record.created_at,
                record.read_at,
                record.decrypt_status,
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<DirectMessageRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at, decrypt_status
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
                    decrypt_status: row.get::<_, Option<String>>(8)?
                        .unwrap_or_else(|| "decrypted".into()),
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list_for_conversation(
        &self,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<DirectMessageRecord>> {
        // Inner query takes the N most recent messages (DESC + LIMIT); outer
        // query re-sorts ASC so the UI can append new sends to the end of the
        // list naturally. Without the wrapper, clients would have to choose
        // between "newest N" and "ascending order" and reverse on the client.
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at, decrypt_status
            FROM (
                SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at, decrypt_status
                FROM direct_messages
                WHERE conversation_id = ?1
                ORDER BY created_at DESC
                LIMIT ?2
            )
            ORDER BY created_at ASC
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
                decrypt_status: row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "decrypted".into()),
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

    fn mark_conversation_read(
        &self,
        conversation_id: &str,
        to_peer_id: &str,
        read_at: &str,
    ) -> Result<usize> {
        // Only mark our incoming messages (where we are the recipient); leaves
        // our own outgoing messages alone since read_at on those is meaningless.
        let updated = self.conn.execute(
            r#"
            UPDATE direct_messages
            SET read_at = ?1
            WHERE conversation_id = ?2
              AND to_peer_id = ?3
              AND read_at IS NULL
            "#,
            params![read_at, conversation_id, to_peer_id],
        )?;
        Ok(updated)
    }

    fn update_decrypt_status(&self, id: &str, status: &str) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE direct_messages
            SET decrypt_status = ?1
            WHERE id = ?2
            "#,
            params![status, id],
        )?;
        Ok(())
    }

    fn list_pending_for_sender(&self, from_peer_id: &str) -> Result<Vec<DirectMessageRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at, decrypt_status
            FROM direct_messages
            WHERE from_peer_id = ?1 AND decrypt_status = 'pending_key'
            ORDER BY created_at ASC
            "#,
        )?;

        let rows = stmt.query_map(params![from_peer_id], |row| {
            Ok(DirectMessageRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                from_peer_id: row.get(2)?,
                to_peer_id: row.get(3)?,
                encrypted_body: row.get(4)?,
                nonce: row.get(5)?,
                created_at: row.get(6)?,
                read_at: row.get(7)?,
                decrypt_status: row
                    .get::<_, Option<String>>(8)?
                    .unwrap_or_else(|| "decrypted".into()),
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
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
