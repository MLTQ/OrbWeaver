use crate::database::models::ConversationRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteConversationRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::ConversationRepository for SqliteConversationRepository<'conn> {
    fn upsert(&self, record: &ConversationRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO conversations (id, peer_id, last_message_at, last_message_preview, unread_count)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                last_message_at = excluded.last_message_at,
                last_message_preview = excluded.last_message_preview,
                unread_count = excluded.unread_count
            "#,
            params![
                record.id,
                record.peer_id,
                record.last_message_at,
                record.last_message_preview,
                record.unread_count
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<ConversationRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, peer_id, last_message_at, last_message_preview, unread_count
            FROM conversations
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(ConversationRecord {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    last_message_at: row.get(2)?,
                    last_message_preview: row.get(3)?,
                    unread_count: row.get(4)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list(&self) -> Result<Vec<ConversationRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, peer_id, last_message_at, last_message_preview, unread_count
            FROM conversations
            ORDER BY last_message_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ConversationRecord {
                id: row.get(0)?,
                peer_id: row.get(1)?,
                last_message_at: row.get(2)?,
                last_message_preview: row.get(3)?,
                unread_count: row.get(4)?,
            })
        })?;

        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(row?);
        }
        Ok(conversations)
    }

    fn update_unread_count(&self, conversation_id: &str, count: i64) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE conversations
            SET unread_count = ?1
            WHERE id = ?2
            "#,
            params![count, conversation_id],
        )?;
        Ok(())
    }

    fn update_last_message(&self, conversation_id: &str, message_at: &str, preview: &str) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE conversations
            SET last_message_at = ?1, last_message_preview = ?2
            WHERE id = ?3
            "#,
            params![message_at, preview, conversation_id],
        )?;
        Ok(())
    }
}
