use crate::database::models::IpBlockRecord;
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub(super) struct SqliteIpBlockRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::IpBlockRepository for SqliteIpBlockRepository<'conn> {
    fn add(&self, record: &IpBlockRecord) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO ip_blocks (ip_or_range, block_type, blocked_at, reason, active, hit_count)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                record.ip_or_range,
                record.block_type,
                record.blocked_at,
                record.reason,
                if record.active { 1 } else { 0 },
                record.hit_count
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    fn remove(&self, id: i64) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM ip_blocks
            WHERE id = ?1
            "#,
            params![id],
        )?;
        Ok(())
    }

    fn set_active(&self, id: i64, active: bool) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE ip_blocks
            SET active = ?1
            WHERE id = ?2
            "#,
            params![if active { 1 } else { 0 }, id],
        )?;
        Ok(())
    }

    fn increment_hit_count(&self, id: i64) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE ip_blocks
            SET hit_count = hit_count + 1
            WHERE id = ?1
            "#,
            params![id],
        )?;
        Ok(())
    }

    fn list_active(&self) -> Result<Vec<IpBlockRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, ip_or_range, block_type, blocked_at, reason, active, hit_count
            FROM ip_blocks
            WHERE active = 1
            ORDER BY blocked_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(IpBlockRecord {
                id: row.get(0)?,
                ip_or_range: row.get(1)?,
                block_type: row.get(2)?,
                blocked_at: row.get(3)?,
                reason: row.get(4)?,
                active: row.get::<_, i64>(5)? != 0,
                hit_count: row.get(6)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    fn list_all(&self) -> Result<Vec<IpBlockRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, ip_or_range, block_type, blocked_at, reason, active, hit_count
            FROM ip_blocks
            ORDER BY blocked_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(IpBlockRecord {
                id: row.get(0)?,
                ip_or_range: row.get(1)?,
                block_type: row.get(2)?,
                blocked_at: row.get(3)?,
                reason: row.get(4)?,
                active: row.get::<_, i64>(5)? != 0,
                hit_count: row.get(6)?,
            })
        })?;

        let mut records = Vec::new();
        for row in rows {
            records.push(row?);
        }
        Ok(records)
    }

    fn get(&self, id: i64) -> Result<Option<IpBlockRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, ip_or_range, block_type, blocked_at, reason, active, hit_count
            FROM ip_blocks
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(IpBlockRecord {
                    id: row.get(0)?,
                    ip_or_range: row.get(1)?,
                    block_type: row.get(2)?,
                    blocked_at: row.get(3)?,
                    reason: row.get(4)?,
                    active: row.get::<_, i64>(5)? != 0,
                    hit_count: row.get(6)?,
                })
            },
        ).optional()?;
        Ok(result)
    }
}
