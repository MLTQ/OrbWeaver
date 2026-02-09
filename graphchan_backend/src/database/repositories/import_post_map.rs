use anyhow::Result;
use rusqlite::{params, Connection};
use std::collections::HashMap;

pub(super) struct SqliteImportPostMapRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::ImportPostMapRepository for SqliteImportPostMapRepository<'conn> {
    fn insert(&self, thread_id: &str, external_id: &str, internal_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR IGNORE INTO import_post_map (thread_id, external_id, internal_id)
            VALUES (?1, ?2, ?3)
            "#,
            params![thread_id, external_id, internal_id],
        )?;
        Ok(())
    }

    fn get_map(&self, thread_id: &str) -> Result<HashMap<String, String>> {
        let mut stmt = self.conn.prepare(
            "SELECT external_id, internal_id FROM import_post_map WHERE thread_id = ?1",
        )?;
        let rows = stmt.query_map(params![thread_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut map = HashMap::new();
        for row in rows {
            let (ext, int) = row?;
            map.insert(ext, int);
        }
        Ok(map)
    }
}
