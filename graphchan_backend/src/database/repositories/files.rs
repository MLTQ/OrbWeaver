use crate::database::models::FileRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteFileRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::FileRepository for SqliteFileRepository<'conn> {
    fn attach(&self, record: &FileRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO files (id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket, download_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                record.id,
                record.post_id,
                record.path,
                record.original_name,
                record.mime,
                record.blob_id,
                record.size_bytes,
                record.checksum,
                record.ticket,
                record.download_status
            ],
        )?;
        Ok(())
    }

    fn upsert(&self, record: &FileRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO files (id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket, download_status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                post_id = excluded.post_id,
                path = excluded.path,
                original_name = excluded.original_name,
                mime = excluded.mime,
                blob_id = excluded.blob_id,
                size_bytes = excluded.size_bytes,
                checksum = excluded.checksum,
                ticket = excluded.ticket,
                download_status = excluded.download_status
            "#,
            params![
                record.id,
                record.post_id,
                record.path,
                record.original_name,
                record.mime,
                record.blob_id,
                record.size_bytes,
                record.checksum,
                record.ticket,
                record.download_status
            ],
        )?;
        Ok(())
    }

    fn list_for_post(&self, post_id: &str) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket, download_status
            FROM files
            WHERE post_id = ?1
            ORDER BY id ASC
            "#,
        )?;
        let rows = stmt.query_map(params![post_id], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                post_id: row.get(1)?,
                path: row.get(2)?,
                original_name: row.get(3)?,
                mime: row.get(4)?,
                blob_id: row.get(5)?,
                size_bytes: row.get(6)?,
                checksum: row.get(7)?,
                ticket: row.get(8)?,
                download_status: row.get(9)?,
            })
        })?;
        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<FileRecord>> {
        tracing::info!("FileRepository::list_for_thread called for thread_id: {}", thread_id);
        let query = r#"
            SELECT f.id, f.post_id, f.path, f.original_name, f.mime, f.blob_id, f.size_bytes, f.checksum, f.ticket, f.download_status
            FROM files f
            INNER JOIN posts p ON f.post_id = p.id
            WHERE p.thread_id = ?1
            ORDER BY f.id ASC
            "#;
        tracing::info!("Preparing query: {}", query);
        let mut stmt = self.conn.prepare(query)
            .map_err(|e| {
                tracing::error!("PREPARE FAILED: {:?}", e);
                e
            })?;
        tracing::info!("Query prepared successfully, executing query_map");
        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(FileRecord {
                id: row.get(0)?,
                post_id: row.get(1)?,
                path: row.get(2)?,
                original_name: row.get(3)?,
                mime: row.get(4)?,
                blob_id: row.get(5)?,
                size_bytes: row.get(6)?,
                checksum: row.get(7)?,
                ticket: row.get(8)?,
                download_status: row.get(9)?,
            })
        })?;
        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }

    fn get(&self, id: &str) -> Result<Option<FileRecord>> {
        Ok(self
            .conn
            .query_row(
                r#"
                SELECT id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket, download_status
                FROM files
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(FileRecord {
                        id: row.get(0)?,
                        post_id: row.get(1)?,
                        path: row.get(2)?,
                        original_name: row.get(3)?,
                        mime: row.get(4)?,
                    blob_id: row.get(5)?,
                        size_bytes: row.get(6)?,
                        checksum: row.get(7)?,
                        ticket: row.get(8)?,
                        download_status: row.get(9)?,
                    })
                },
            )
            .optional()?)
    }
}
