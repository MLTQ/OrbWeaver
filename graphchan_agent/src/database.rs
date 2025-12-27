use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportantPost {
    pub id: String,
    pub post_id: String,
    pub thread_id: String,
    pub post_body: String,
    pub why_important: String,
    pub importance_score: f64, // 0.0-1.0, how formative this experience was
    pub marked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReflectionRecord {
    pub id: String,
    pub reflected_at: DateTime<Utc>,
    pub old_prompt: String,
    pub new_prompt: String,
    pub reasoning: String,
    pub guiding_principles: String, // JSON array
}

pub struct AgentDatabase {
    conn: Connection,
}

impl AgentDatabase {
    /// Create or open the database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn };
        db.ensure_schema()?;
        Ok(db)
    }

    /// Create the database schema
    fn ensure_schema(&self) -> Result<()> {
        self.conn.execute(
            r#"CREATE TABLE IF NOT EXISTS important_posts (
                id TEXT PRIMARY KEY,
                post_id TEXT NOT NULL,
                thread_id TEXT NOT NULL,
                post_body TEXT NOT NULL,
                why_important TEXT NOT NULL,
                importance_score REAL NOT NULL,
                marked_at TEXT NOT NULL
            )"#,
            [],
        )?;

        self.conn.execute(
            r#"CREATE TABLE IF NOT EXISTS reflection_history (
                id TEXT PRIMARY KEY,
                reflected_at TEXT NOT NULL,
                old_prompt TEXT NOT NULL,
                new_prompt TEXT NOT NULL,
                reasoning TEXT NOT NULL,
                guiding_principles TEXT NOT NULL
            )"#,
            [],
        )?;

        self.conn.execute(
            r#"CREATE TABLE IF NOT EXISTS agent_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"#,
            [],
        )?;

        // Create index for faster lookups
        self.conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_important_posts_marked_at ON important_posts(marked_at DESC)",
            [],
        )?;

        Ok(())
    }

    /// Save an important post
    pub fn save_important_post(&self, post: &ImportantPost) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO important_posts (id, post_id, thread_id, post_body, why_important, importance_score, marked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                post.id,
                post.post_id,
                post.thread_id,
                post.post_body,
                post.why_important,
                post.importance_score,
                post.marked_at.to_rfc3339()
            ],
        )?;
        Ok(())
    }

    /// Get the N most recent important posts
    pub fn get_recent_important_posts(&self, limit: usize) -> Result<Vec<ImportantPost>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, post_id, thread_id, post_body, why_important, importance_score, marked_at
             FROM important_posts
             ORDER BY marked_at DESC
             LIMIT ?1",
        )?;

        let posts = stmt
            .query_map([limit], |row| {
                Ok(ImportantPost {
                    id: row.get(0)?,
                    post_id: row.get(1)?,
                    thread_id: row.get(2)?,
                    post_body: row.get(3)?,
                    why_important: row.get(4)?,
                    importance_score: row.get(5)?,
                    marked_at: row
                        .get::<_, String>(6)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(posts)
    }

    /// Count total important posts
    pub fn count_important_posts(&self) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM important_posts",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Get all important posts ordered by score (lowest first)
    pub fn get_all_important_posts_by_score(&self) -> Result<Vec<ImportantPost>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, post_id, thread_id, post_body, why_important, importance_score, marked_at
             FROM important_posts
             ORDER BY importance_score ASC",
        )?;

        let posts = stmt
            .query_map([], |row| {
                Ok(ImportantPost {
                    id: row.get(0)?,
                    post_id: row.get(1)?,
                    thread_id: row.get(2)?,
                    post_body: row.get(3)?,
                    why_important: row.get(4)?,
                    importance_score: row.get(5)?,
                    marked_at: row
                        .get::<_, String>(6)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            6,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(posts)
    }

    /// Delete an important post by ID
    pub fn delete_important_post(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM important_posts WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Save a reflection record
    pub fn save_reflection(&self, reflection: &ReflectionRecord) -> Result<()> {
        self.conn.execute(
            "INSERT INTO reflection_history (id, reflected_at, old_prompt, new_prompt, reasoning, guiding_principles)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                reflection.id,
                reflection.reflected_at.to_rfc3339(),
                reflection.old_prompt,
                reflection.new_prompt,
                reflection.reasoning,
                reflection.guiding_principles
            ],
        )?;
        Ok(())
    }

    /// Get reflection history
    pub fn get_reflection_history(&self, limit: usize) -> Result<Vec<ReflectionRecord>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, reflected_at, old_prompt, new_prompt, reasoning, guiding_principles
             FROM reflection_history
             ORDER BY reflected_at DESC
             LIMIT ?1",
        )?;

        let reflections = stmt
            .query_map([limit], |row| {
                Ok(ReflectionRecord {
                    id: row.get(0)?,
                    reflected_at: row
                        .get::<_, String>(1)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    old_prompt: row.get(2)?,
                    new_prompt: row.get(3)?,
                    reasoning: row.get(4)?,
                    guiding_principles: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(reflections)
    }

    /// Get a state value
    pub fn get_state(&self, key: &str) -> Result<Option<String>> {
        let result = self.conn.query_row(
            "SELECT value FROM agent_state WHERE key = ?1",
            [key],
            |row| row.get(0),
        );

        match result {
            Ok(value) => Ok(Some(value)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Set a state value
    pub fn set_state(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT OR REPLACE INTO agent_state (key, value) VALUES (?1, ?2)",
            params![key, value],
        )?;
        Ok(())
    }

    /// Get current system prompt
    pub fn get_current_system_prompt(&self) -> Result<Option<String>> {
        self.get_state("current_system_prompt")
    }

    /// Set current system prompt
    pub fn set_current_system_prompt(&self, prompt: &str) -> Result<()> {
        self.set_state("current_system_prompt", prompt)
    }

    /// Get last reflection time
    pub fn get_last_reflection_time(&self) -> Result<Option<DateTime<Utc>>> {
        if let Some(time_str) = self.get_state("last_reflection_time")? {
            let time: DateTime<Utc> = time_str.parse().context("Failed to parse reflection time")?;
            Ok(Some(time))
        } else {
            Ok(None)
        }
    }

    /// Set last reflection time
    pub fn set_last_reflection_time(&self, time: DateTime<Utc>) -> Result<()> {
        self.set_state("last_reflection_time", &time.to_rfc3339())
    }
}
