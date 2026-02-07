use anyhow::Result;
use rusqlite::{params, Connection};

pub(super) struct SqliteTopicRepository<'conn> {
    pub(super) conn: &'conn Connection,
}

impl<'conn> super::TopicRepository for SqliteTopicRepository<'conn> {
    fn subscribe(&self, topic_id: &str) -> Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT OR IGNORE INTO user_topics (topic_id, subscribed_at) VALUES (?1, ?2)",
            params![topic_id, now],
        )?;
        Ok(())
    }

    fn unsubscribe(&self, topic_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM user_topics WHERE topic_id = ?1",
            params![topic_id],
        )?;
        Ok(())
    }

    fn list_subscribed(&self) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT topic_id FROM user_topics ORDER BY subscribed_at DESC"
        )?;
        let topics = stmt.query_map([], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(topics)
    }

    fn is_subscribed(&self, topic_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM user_topics WHERE topic_id = ?1",
            params![topic_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn add_thread_topic(&self, thread_id: &str, topic_id: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR IGNORE INTO thread_topics (thread_id, topic_id) VALUES (?1, ?2)",
            params![thread_id, topic_id],
        )?;
        Ok(())
    }

    fn remove_thread_topic(&self, thread_id: &str, topic_id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM thread_topics WHERE thread_id = ?1 AND topic_id = ?2",
            params![thread_id, topic_id],
        )?;
        Ok(())
    }

    fn list_thread_topics(&self, thread_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT topic_id FROM thread_topics WHERE thread_id = ?1"
        )?;
        let topics = stmt.query_map(params![thread_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(topics)
    }

    fn list_threads_for_topic(&self, topic_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            "SELECT thread_id FROM thread_topics WHERE topic_id = ?1"
        )?;
        let threads = stmt.query_map(params![topic_id], |row| row.get(0))?
            .collect::<Result<Vec<String>, _>>()?;
        Ok(threads)
    }
}
