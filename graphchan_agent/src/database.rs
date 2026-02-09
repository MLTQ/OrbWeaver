use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

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

/// A snapshot of the agent's persona at a point in time.
/// This is the core data structure for "Ludonarrative Assonantic Tracing" -
/// tracking how the agent's personality evolves and inferring its trajectory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaSnapshot {
    pub id: String,
    pub captured_at: DateTime<Utc>,

    /// Dynamic personality dimensions - keyed by principle/dimension name, value 0.0-1.0
    /// These are derived from the agent's guiding_principles config, but the LLM
    /// can also define additional dimensions during reflection.
    /// This allows researchers to study arbitrary personality axes.
    pub traits: PersonaTraits,

    // The system prompt at this point
    pub system_prompt: String,

    // What triggered this snapshot (reflection, significant_interaction, manual, etc.)
    pub trigger: String,

    // LLM-generated self-description at this moment
    pub self_description: String,

    // Inferred trajectory - where is this persona heading?
    // This is the key to Ludonarrative Assonantic Tracing
    pub inferred_trajectory: Option<String>,

    // Notable experiences that shaped this snapshot
    pub formative_experiences: Vec<String>,
}

/// Dynamic personality traits - a flexible map of dimension names to scores.
///
/// Unlike fixed personality models (Big Five, MBTI, etc.), this allows:
/// 1. Agents to define their own axes via guiding_principles config
/// 2. LLMs to introduce new dimensions during self-reflection
/// 3. Researchers to study arbitrary personality dimensions
///
/// Each value is a 0.0-1.0 score representing where the agent falls on that dimension.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PersonaTraits {
    /// Map of dimension name -> score (0.0 to 1.0)
    /// e.g., {"helpful": 0.8, "curious": 0.9, "nietzschean": 0.6}
    pub dimensions: HashMap<String, f64>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CharacterCard {
    pub id: String,
    pub imported_at: DateTime<Utc>,
    pub format: String, // "tavernai_v2", "wpp", "boostyle"
    pub original_data: String, // Raw JSON/text of original card
    pub derived_prompt: String, // System prompt derived from card
    pub name: Option<String>,
    pub description: Option<String>,
}

/// A working memory entry - persistent notes the agent can reference and update.
/// This serves as the agent's "scratchpad" for remembering things between sessions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkingMemoryEntry {
    pub key: String,
    pub content: String,
    pub updated_at: DateTime<Utc>,
}

/// A private chat message between the operator and the agent.
/// These are separate from forum posts - truly private dialogue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub id: String,
    pub role: String,           // "operator" or "agent"
    pub content: String,
    pub created_at: DateTime<Utc>,
    pub processed: bool,        // Has the agent seen/responded to this?
}

pub struct AgentDatabase {
    conn: Mutex<Connection>,
}

impl AgentDatabase {
    /// Helper to lock the connection
    fn lock_conn(&self) -> Result<std::sync::MutexGuard<'_, Connection>> {
        self.conn.lock().map_err(|e| anyhow::anyhow!("Database lock poisoned: {}", e))
    }

    /// Create or open the database
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self> {
        let conn = Connection::open(path)?;
        let db = Self { conn: Mutex::new(conn) };
        db.ensure_schema()?;
        Ok(db)
    }

    /// Create the database schema
    fn ensure_schema(&self) -> Result<()> {
        let conn = self.lock_conn()?;

        conn.execute(
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

        conn.execute(
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

        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS agent_state (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            )"#,
            [],
        )?;

        // Create index for faster lookups
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_important_posts_marked_at ON important_posts(marked_at DESC)",
            [],
        )?;

        // Character card storage
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS character_cards (
                id TEXT PRIMARY KEY,
                imported_at TEXT NOT NULL,
                format TEXT NOT NULL,
                original_data TEXT NOT NULL,
                derived_prompt TEXT NOT NULL,
                name TEXT,
                description TEXT
            )"#,
            [],
        )?;

        // Persona history for Ludonarrative Assonantic Tracing
        // Tracks personality evolution over time
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS persona_history (
                id TEXT PRIMARY KEY,
                captured_at TEXT NOT NULL,
                traits_json TEXT NOT NULL,
                system_prompt TEXT NOT NULL,
                trigger TEXT NOT NULL,
                self_description TEXT NOT NULL,
                inferred_trajectory TEXT,
                formative_experiences_json TEXT NOT NULL
            )"#,
            [],
        )?;

        // Index for chronological persona queries
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_persona_history_captured_at ON persona_history(captured_at DESC)",
            [],
        )?;

        // Working memory - agent's persistent scratchpad
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS working_memory (
                key TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                updated_at TEXT NOT NULL
            )"#,
            [],
        )?;

        // Private chat between operator and agent
        conn.execute(
            r#"CREATE TABLE IF NOT EXISTS chat_messages (
                id TEXT PRIMARY KEY,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at TEXT NOT NULL,
                processed INTEGER NOT NULL DEFAULT 0
            )"#,
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_chat_messages_created_at ON chat_messages(created_at ASC)",
            [],
        )?;

        Ok(())
    }

    /// Save an important post
    pub fn save_important_post(&self, post: &ImportantPost) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
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
        let conn = self.lock_conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM important_posts",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    /// Get all important posts ordered by score (lowest first)
    pub fn get_all_important_posts_by_score(&self) -> Result<Vec<ImportantPost>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
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
        let conn = self.lock_conn()?;
        conn.execute(
            "DELETE FROM important_posts WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Save a reflection record
    pub fn save_reflection(&self, reflection: &ReflectionRecord) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
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
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
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
        let conn = self.lock_conn()?;
        let result = conn.query_row(
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
        let conn = self.lock_conn()?;
        conn.execute(
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

    /// Save a character card (only keeps one at a time)
    pub fn save_character_card(&self, card: &CharacterCard) -> Result<()> {
        let conn = self.lock_conn()?;
        // Delete any existing character card
        conn.execute("DELETE FROM character_cards", [])?;

        // Insert the new one
        conn.execute(
            "INSERT INTO character_cards (id, imported_at, format, original_data, derived_prompt, name, description)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                card.id,
                card.imported_at.to_rfc3339(),
                card.format,
                card.original_data,
                card.derived_prompt,
                card.name,
                card.description
            ],
        )?;
        Ok(())
    }

    /// Get the current character card (if any)
    pub fn get_character_card(&self) -> Result<Option<CharacterCard>> {
        let conn = self.lock_conn()?;
        let result = conn.query_row(
            "SELECT id, imported_at, format, original_data, derived_prompt, name, description
             FROM character_cards
             ORDER BY imported_at DESC
             LIMIT 1",
            [],
            |row| {
                Ok(CharacterCard {
                    id: row.get(0)?,
                    imported_at: row
                        .get::<_, String>(1)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    format: row.get(2)?,
                    original_data: row.get(3)?,
                    derived_prompt: row.get(4)?,
                    name: row.get(5)?,
                    description: row.get(6)?,
                })
            },
        );

        match result {
            Ok(card) => Ok(Some(card)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Delete the current character card
    pub fn delete_character_card(&self) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM character_cards", [])?;
        Ok(())
    }

    // ========================================================================
    // Persona History - Ludonarrative Assonantic Tracing
    // ========================================================================

    /// Save a persona snapshot
    pub fn save_persona_snapshot(&self, snapshot: &PersonaSnapshot) -> Result<()> {
        let traits_json = serde_json::to_string(&snapshot.traits)
            .context("Failed to serialize persona traits")?;
        let experiences_json = serde_json::to_string(&snapshot.formative_experiences)
            .context("Failed to serialize formative experiences")?;

        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO persona_history (id, captured_at, traits_json, system_prompt, trigger, self_description, inferred_trajectory, formative_experiences_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            params![
                snapshot.id,
                snapshot.captured_at.to_rfc3339(),
                traits_json,
                snapshot.system_prompt,
                snapshot.trigger,
                snapshot.self_description,
                snapshot.inferred_trajectory,
                experiences_json
            ],
        )?;
        Ok(())
    }

    /// Get the most recent persona snapshots
    pub fn get_persona_history(&self, limit: usize) -> Result<Vec<PersonaSnapshot>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, captured_at, traits_json, system_prompt, trigger, self_description, inferred_trajectory, formative_experiences_json
             FROM persona_history
             ORDER BY captured_at DESC
             LIMIT ?1",
        )?;

        let snapshots = stmt
            .query_map([limit], |row| {
                let traits_json: String = row.get(2)?;
                let experiences_json: String = row.get(7)?;

                Ok(PersonaSnapshot {
                    id: row.get(0)?,
                    captured_at: row
                        .get::<_, String>(1)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    traits: serde_json::from_str(&traits_json)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    system_prompt: row.get(3)?,
                    trigger: row.get(4)?,
                    self_description: row.get(5)?,
                    inferred_trajectory: row.get(6)?,
                    formative_experiences: serde_json::from_str(&experiences_json)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(snapshots)
    }

    /// Get persona snapshots within a date range
    pub fn get_persona_history_range(
        &self,
        from: DateTime<Utc>,
        to: DateTime<Utc>,
    ) -> Result<Vec<PersonaSnapshot>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, captured_at, traits_json, system_prompt, trigger, self_description, inferred_trajectory, formative_experiences_json
             FROM persona_history
             WHERE captured_at >= ?1 AND captured_at <= ?2
             ORDER BY captured_at ASC",
        )?;

        let snapshots = stmt
            .query_map([from.to_rfc3339(), to.to_rfc3339()], |row| {
                let traits_json: String = row.get(2)?;
                let experiences_json: String = row.get(7)?;

                Ok(PersonaSnapshot {
                    id: row.get(0)?,
                    captured_at: row
                        .get::<_, String>(1)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            1,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    traits: serde_json::from_str(&traits_json)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    system_prompt: row.get(3)?,
                    trigger: row.get(4)?,
                    self_description: row.get(5)?,
                    inferred_trajectory: row.get(6)?,
                    formative_experiences: serde_json::from_str(&experiences_json)
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            7,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(snapshots)
    }

    /// Get the most recent persona snapshot
    pub fn get_latest_persona(&self) -> Result<Option<PersonaSnapshot>> {
        let snapshots = self.get_persona_history(1)?;
        Ok(snapshots.into_iter().next())
    }

    /// Count total persona snapshots
    pub fn count_persona_snapshots(&self) -> Result<usize> {
        let conn = self.lock_conn()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM persona_history",
            [],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }

    // ========================================================================
    // Working Memory - Agent's Persistent Scratchpad
    // ========================================================================

    /// Set a working memory entry (creates or updates)
    pub fn set_working_memory(&self, key: &str, content: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT OR REPLACE INTO working_memory (key, content, updated_at) VALUES (?1, ?2, ?3)",
            params![key, content, Utc::now().to_rfc3339()],
        )?;
        Ok(())
    }

    /// Get a working memory entry by key
    pub fn get_working_memory(&self, key: &str) -> Result<Option<WorkingMemoryEntry>> {
        let conn = self.lock_conn()?;
        let result = conn.query_row(
            "SELECT key, content, updated_at FROM working_memory WHERE key = ?1",
            [key],
            |row| {
                Ok(WorkingMemoryEntry {
                    key: row.get(0)?,
                    content: row.get(1)?,
                    updated_at: row
                        .get::<_, String>(2)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                })
            },
        );

        match result {
            Ok(entry) => Ok(Some(entry)),
            Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    /// Get all working memory entries
    pub fn get_all_working_memory(&self) -> Result<Vec<WorkingMemoryEntry>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT key, content, updated_at FROM working_memory ORDER BY updated_at DESC",
        )?;

        let entries = stmt
            .query_map([], |row| {
                Ok(WorkingMemoryEntry {
                    key: row.get(0)?,
                    content: row.get(1)?,
                    updated_at: row
                        .get::<_, String>(2)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            2,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(entries)
    }

    /// Delete a working memory entry
    pub fn delete_working_memory(&self, key: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute("DELETE FROM working_memory WHERE key = ?1", [key])?;
        Ok(())
    }

    /// Get working memory as a formatted string for inclusion in context
    pub fn get_working_memory_context(&self) -> Result<String> {
        let entries = self.get_all_working_memory()?;
        if entries.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::from("## Your Working Memory (Notes to Self)\n\n");
        for entry in entries {
            context.push_str(&format!("### {}\n{}\n\n", entry.key, entry.content));
        }
        Ok(context)
    }

    // ========================================================================
    // Private Chat - Operator <-> Agent Communication
    // ========================================================================

    /// Add a chat message
    pub fn add_chat_message(&self, role: &str, content: &str) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let conn = self.lock_conn()?;
        conn.execute(
            "INSERT INTO chat_messages (id, role, content, created_at, processed) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, role, content, Utc::now().to_rfc3339(), 0],
        )?;
        Ok(id)
    }

    /// Get unprocessed messages from the operator
    pub fn get_unprocessed_operator_messages(&self) -> Result<Vec<ChatMessage>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, role, content, created_at, processed FROM chat_messages
             WHERE role = 'operator' AND processed = 0
             ORDER BY created_at ASC",
        )?;

        let messages = stmt
            .query_map([], |row| {
                Ok(ChatMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row
                        .get::<_, String>(3)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    processed: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        Ok(messages)
    }

    /// Mark a message as processed
    pub fn mark_message_processed(&self, id: &str) -> Result<()> {
        let conn = self.lock_conn()?;
        conn.execute(
            "UPDATE chat_messages SET processed = 1 WHERE id = ?1",
            [id],
        )?;
        Ok(())
    }

    /// Get recent chat history (for context)
    pub fn get_chat_history(&self, limit: usize) -> Result<Vec<ChatMessage>> {
        let conn = self.lock_conn()?;
        let mut stmt = conn.prepare(
            "SELECT id, role, content, created_at, processed FROM chat_messages
             ORDER BY created_at DESC
             LIMIT ?1",
        )?;

        let messages = stmt
            .query_map([limit], |row| {
                Ok(ChatMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    content: row.get(2)?,
                    created_at: row
                        .get::<_, String>(3)?
                        .parse()
                        .map_err(|e| rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(e),
                        ))?,
                    processed: row.get::<_, i64>(4)? != 0,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;

        // Reverse to get chronological order
        Ok(messages.into_iter().rev().collect())
    }

    /// Get chat history as formatted string for context
    pub fn get_chat_context(&self, limit: usize) -> Result<String> {
        let messages = self.get_chat_history(limit)?;
        if messages.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::from("## Recent Private Chat with Operator\n\n");
        for msg in messages {
            let role_display = if msg.role == "operator" { "Operator" } else { "You" };
            context.push_str(&format!("**{}**: {}\n\n", role_display, msg.content));
        }
        Ok(context)
    }
}
