pub mod models;
pub mod repositories;

use crate::config::GraphchanPaths;
use anyhow::{anyhow, Context, Result};
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::{Arc, Mutex};

pub(crate) const MIGRATIONS: &str = r#"
    PRAGMA journal_mode = WAL;
    PRAGMA foreign_keys = ON;

    CREATE TABLE IF NOT EXISTS settings (
        key TEXT PRIMARY KEY,
        value TEXT NOT NULL
    );

    CREATE TABLE IF NOT EXISTS node_identity (
        id INTEGER PRIMARY KEY CHECK (id = 1),
        gpg_fingerprint TEXT,
        iroh_peer_id TEXT,
        friendcode TEXT
    );

    CREATE TABLE IF NOT EXISTS peers (
        id TEXT PRIMARY KEY,
        alias TEXT,
        friendcode TEXT,
        iroh_peer_id TEXT UNIQUE,
        gpg_fingerprint TEXT,
        last_seen TEXT,
        trust_state TEXT DEFAULT 'unknown'
    );

    CREATE TABLE IF NOT EXISTS threads (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        creator_peer_id TEXT,
        created_at TEXT NOT NULL,
        pinned INTEGER DEFAULT 0,
        FOREIGN KEY (creator_peer_id) REFERENCES peers(id)
    );

    CREATE TABLE IF NOT EXISTS posts (
        id TEXT PRIMARY KEY,
        thread_id TEXT NOT NULL,
        author_peer_id TEXT,
        body TEXT NOT NULL,
        created_at TEXT NOT NULL,
        updated_at TEXT,
        FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE,
        FOREIGN KEY (author_peer_id) REFERENCES peers(id)
    );

    CREATE TABLE IF NOT EXISTS post_relationships (
        parent_id TEXT NOT NULL,
        child_id TEXT NOT NULL,
        PRIMARY KEY (parent_id, child_id),
        FOREIGN KEY (parent_id) REFERENCES posts(id) ON DELETE CASCADE,
        FOREIGN KEY (child_id) REFERENCES posts(id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS files (
        id TEXT PRIMARY KEY,
        post_id TEXT NOT NULL,
        path TEXT NOT NULL,
        original_name TEXT,
        mime TEXT,
        blob_id TEXT,
        size_bytes INTEGER,
        checksum TEXT,
        FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_posts_thread ON posts(thread_id);
    CREATE INDEX IF NOT EXISTS idx_post_relationships_child ON post_relationships(child_id);
    CREATE INDEX IF NOT EXISTS idx_files_post ON files(post_id);
"#;

#[derive(Clone)]
pub struct Database {
    conn: Arc<Mutex<Connection>>,
    newly_created: bool,
}

impl Database {
    pub fn connect(paths: &GraphchanPaths) -> Result<Self> {
        let newly_created = !paths.db_path.exists();
        let conn = Connection::open(&paths.db_path)?;
        Ok(Self::from_connection(conn, newly_created))
    }

    pub fn from_connection(conn: Connection, newly_created: bool) -> Self {
        Self {
            conn: Arc::new(Mutex::new(conn)),
            newly_created,
        }
    }

    pub fn ensure_migrations(&self) -> Result<bool> {
        self.with_conn(|conn| {
            conn.execute_batch(MIGRATIONS)?;
            self.ensure_node_identity_schema_locked(conn)?;
            self.ensure_files_schema_locked(conn)?;
            Ok(())
        })?;
        Ok(self.newly_created)
    }

    pub fn save_identity(
        &self,
        fingerprint: &str,
        iroh_peer_id: &str,
        friendcode: &str,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO node_identity (id, gpg_fingerprint, iroh_peer_id, friendcode)
                VALUES (1, ?, ?, ?)
                ON CONFLICT(id) DO UPDATE SET
                    gpg_fingerprint = excluded.gpg_fingerprint,
                    iroh_peer_id = excluded.iroh_peer_id,
                    friendcode = excluded.friendcode;
                "#,
                params![fingerprint, iroh_peer_id, friendcode],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn with_repositories<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(repositories::SqliteRepositories<'_>) -> Result<T>,
    {
        self.with_conn(|conn| {
            let repos = repositories::SqliteRepositories::new(conn);
            f(repos)
        })
    }

    fn ensure_node_identity_schema_locked(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(node_identity)")?;
        let mut has_friendcode = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("friendcode") {
                has_friendcode = true;
                break;
            }
        }
        if !has_friendcode {
            conn.execute("ALTER TABLE node_identity ADD COLUMN friendcode TEXT", [])?;
        }
        Ok(())
    }

    fn ensure_files_schema_locked(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(files)")?;
        let mut has_original_name = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("original_name") {
                has_original_name = true;
                break;
            }
        }
        if !has_original_name {
            conn.execute("ALTER TABLE files ADD COLUMN original_name TEXT", [])?;
        }
        Ok(())
    }

    pub fn upsert_local_peer(
        &self,
        fingerprint: &str,
        peer_id: &str,
        friendcode: &str,
    ) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                r#"
                INSERT INTO peers (id, alias, friendcode, iroh_peer_id, gpg_fingerprint, last_seen, trust_state)
                VALUES (?1, 'local', ?2, ?3, ?1, datetime('now'), 'trusted')
                ON CONFLICT(id) DO UPDATE SET
                    friendcode = excluded.friendcode,
                    iroh_peer_id = excluded.iroh_peer_id,
                    gpg_fingerprint = excluded.gpg_fingerprint,
                    last_seen = excluded.last_seen,
                    trust_state = excluded.trust_state
                "#,
                params![fingerprint, friendcode, peer_id],
            )?;
            Ok(())
        })?;
        Ok(())
    }

    pub fn get_identity(&self) -> Result<Option<(String, String, String)>> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT gpg_fingerprint, iroh_peer_id, friendcode FROM node_identity WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()
            .context("failed to load node_identity row")
        })
    }

    fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("database mutex poisoned"))?;
        f(&guard)
    }
}
