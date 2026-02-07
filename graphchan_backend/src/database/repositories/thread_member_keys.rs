use crate::database::models::ThreadMemberKey;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteThreadMemberKeyRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::ThreadMemberKeyRepository for SqliteThreadMemberKeyRepository<'conn> {
    fn add(&self, record: &ThreadMemberKey) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO thread_member_keys (thread_id, member_peer_id, wrapped_key_ciphertext, wrapped_key_nonce)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                record.thread_id,
                record.member_peer_id,
                record.wrapped_key_ciphertext,
                record.wrapped_key_nonce
            ],
        )?;
        Ok(())
    }

    fn get(&self, thread_id: &str, member_peer_id: &str) -> Result<Option<ThreadMemberKey>> {
        let result = self.conn.query_row(
            r#"
            SELECT thread_id, member_peer_id, wrapped_key_ciphertext, wrapped_key_nonce
            FROM thread_member_keys
            WHERE thread_id = ?1 AND member_peer_id = ?2
            "#,
            params![thread_id, member_peer_id],
            |row| {
                Ok(ThreadMemberKey {
                    thread_id: row.get(0)?,
                    member_peer_id: row.get(1)?,
                    wrapped_key_ciphertext: row.get(2)?,
                    wrapped_key_nonce: row.get(3)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<ThreadMemberKey>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT thread_id, member_peer_id, wrapped_key_ciphertext, wrapped_key_nonce
            FROM thread_member_keys
            WHERE thread_id = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(ThreadMemberKey {
                thread_id: row.get(0)?,
                member_peer_id: row.get(1)?,
                wrapped_key_ciphertext: row.get(2)?,
                wrapped_key_nonce: row.get(3)?,
            })
        })?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(row?);
        }
        Ok(keys)
    }

    fn remove(&self, thread_id: &str, member_peer_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM thread_member_keys
            WHERE thread_id = ?1 AND member_peer_id = ?2
            "#,
            params![thread_id, member_peer_id],
        )?;
        Ok(())
    }
}
