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
        trust_state TEXT DEFAULT 'unknown',
        avatar_file_id TEXT,
        username TEXT,
        bio TEXT
    );

    CREATE TABLE IF NOT EXISTS threads (
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        creator_peer_id TEXT,
        created_at TEXT NOT NULL,
        pinned INTEGER DEFAULT 0,
        thread_hash TEXT,
        rebroadcast INTEGER DEFAULT 1,
        deleted INTEGER DEFAULT 0,
        ignored INTEGER DEFAULT 0,
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
        ticket TEXT,
        FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE
    );

    CREATE INDEX IF NOT EXISTS idx_posts_thread ON posts(thread_id);
    CREATE INDEX IF NOT EXISTS idx_post_relationships_child ON post_relationships(child_id);
    CREATE INDEX IF NOT EXISTS idx_files_post ON files(post_id);

    CREATE TABLE IF NOT EXISTS thread_tickets (
        thread_id TEXT PRIMARY KEY,
        ticket TEXT NOT NULL,
        FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE
    );

    CREATE TABLE IF NOT EXISTS reactions (
        post_id TEXT NOT NULL,
        reactor_peer_id TEXT NOT NULL,
        emoji TEXT NOT NULL,
        signature TEXT NOT NULL,
        created_at TEXT NOT NULL,
        PRIMARY KEY (post_id, reactor_peer_id, emoji),
        FOREIGN KEY (post_id) REFERENCES posts(id) ON DELETE CASCADE,
        FOREIGN KEY (reactor_peer_id) REFERENCES peers(id)
    );

    CREATE INDEX IF NOT EXISTS idx_reactions_post ON reactions(post_id);

    -- Migrations for new fields
    -- ALTER TABLE peers ADD COLUMN avatar_file_id TEXT;
    -- ALTER TABLE peers ADD COLUMN username TEXT;
    -- ALTER TABLE peers ADD COLUMN bio TEXT;
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
            self.ensure_avatar_column(conn)?;
            self.ensure_thread_blob_ticket_column(conn)?;
            self.ensure_thread_hash_column(conn)?;
            self.ensure_x25519_pubkey_column(conn)?;
            self.ensure_thread_visibility_columns(conn)?;
            self.ensure_thread_member_keys_table(conn)?;
            self.ensure_dm_tables(conn)?;
            self.ensure_blocking_tables(conn)?;
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
        let mut has_ticket = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("original_name") {
                has_original_name = true;
            }
            if name.eq_ignore_ascii_case("ticket") {
                has_ticket = true;
            }
        }
        if !has_original_name {
            conn.execute("ALTER TABLE files ADD COLUMN original_name TEXT", [])?;
        }
        if !has_ticket {
            conn.execute("ALTER TABLE files ADD COLUMN ticket TEXT", [])?;
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
    fn ensure_avatar_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(peers)")?;
        let mut has_avatar = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("avatar_file_id") {
                has_avatar = true;
                break;
            }
        }
        if !has_avatar {
            conn.execute("ALTER TABLE peers ADD COLUMN avatar_file_id TEXT", [])?;
        }
        Ok(())
    }

    fn ensure_thread_blob_ticket_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(threads)")?;
        let mut has_blob_ticket = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            if row? == "blob_ticket" {
                has_blob_ticket = true;
                break;
            }
        }
        if !has_blob_ticket {
            conn.execute("ALTER TABLE threads ADD COLUMN blob_ticket TEXT", [])?;
        }
        Ok(())
    }

    fn ensure_thread_hash_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(threads)")?;
        let mut has_thread_hash = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            if row? == "thread_hash" {
                has_thread_hash = true;
                break;
            }
        }
        if !has_thread_hash {
            conn.execute("ALTER TABLE threads ADD COLUMN thread_hash TEXT", [])?;
        }
        Ok(())
    }

    fn ensure_x25519_pubkey_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(peers)")?;
        let mut has_x25519_pubkey = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("x25519_pubkey") {
                has_x25519_pubkey = true;
                break;
            }
        }
        if !has_x25519_pubkey {
            conn.execute("ALTER TABLE peers ADD COLUMN x25519_pubkey TEXT", [])?;
        }
        Ok(())
    }

    fn ensure_thread_visibility_columns(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(threads)")?;
        let mut has_visibility = false;
        let mut has_topic_secret = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("visibility") {
                has_visibility = true;
            }
            if name.eq_ignore_ascii_case("topic_secret") {
                has_topic_secret = true;
            }
        }
        if !has_visibility {
            // Default to 'social' for existing threads
            conn.execute("ALTER TABLE threads ADD COLUMN visibility TEXT DEFAULT 'social'", [])?;
        }
        if !has_topic_secret {
            conn.execute("ALTER TABLE threads ADD COLUMN topic_secret TEXT", [])?;
        }
        Ok(())
    }

    fn ensure_thread_member_keys_table(&self, conn: &Connection) -> Result<()> {
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS thread_member_keys (
                thread_id TEXT NOT NULL,
                member_peer_id TEXT NOT NULL,
                wrapped_key_ciphertext BLOB NOT NULL,
                wrapped_key_nonce BLOB NOT NULL,
                PRIMARY KEY (thread_id, member_peer_id),
                FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE,
                FOREIGN KEY (member_peer_id) REFERENCES peers(id) ON DELETE CASCADE
            )
            "#,
            [],
        )?;
        Ok(())
    }

    fn ensure_dm_tables(&self, conn: &Connection) -> Result<()> {
        // Create direct_messages table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS direct_messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL,
                from_peer_id TEXT NOT NULL,
                to_peer_id TEXT NOT NULL,
                encrypted_body BLOB NOT NULL,
                nonce BLOB NOT NULL,
                created_at TEXT NOT NULL,
                read_at TEXT,
                FOREIGN KEY (from_peer_id) REFERENCES peers(id),
                FOREIGN KEY (to_peer_id) REFERENCES peers(id)
            )
            "#,
            [],
        )?;

        // Create index for conversation queries
        conn.execute(
            r#"
            CREATE INDEX IF NOT EXISTS idx_dm_conversation
            ON direct_messages(conversation_id, created_at)
            "#,
            [],
        )?;

        // Create index for unread messages
        conn.execute(
            r#"
            CREATE INDEX IF NOT EXISTS idx_dm_unread
            ON direct_messages(to_peer_id, read_at)
            WHERE read_at IS NULL
            "#,
            [],
        )?;

        // Create conversations table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS conversations (
                id TEXT PRIMARY KEY,
                peer_id TEXT NOT NULL,
                last_message_at TEXT,
                last_message_preview TEXT,
                unread_count INTEGER DEFAULT 0,
                FOREIGN KEY (peer_id) REFERENCES peers(id)
            )
            "#,
            [],
        )?;

        Ok(())
    }

    fn ensure_blocking_tables(&self, conn: &Connection) -> Result<()> {
        // Create blocked_peers table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS blocked_peers (
                peer_id TEXT PRIMARY KEY,
                reason TEXT,
                blocked_at TEXT NOT NULL,
                FOREIGN KEY (peer_id) REFERENCES peers(id)
            )
            "#,
            [],
        )?;

        // Create blocklist_subscriptions table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS blocklist_subscriptions (
                id TEXT PRIMARY KEY,
                maintainer_peer_id TEXT NOT NULL,
                name TEXT NOT NULL,
                description TEXT,
                auto_apply INTEGER DEFAULT 1,
                last_synced_at TEXT,
                FOREIGN KEY (maintainer_peer_id) REFERENCES peers(id)
            )
            "#,
            [],
        )?;

        // Create blocklist_entries table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS blocklist_entries (
                blocklist_id TEXT NOT NULL,
                peer_id TEXT NOT NULL,
                reason TEXT,
                added_at TEXT NOT NULL,
                PRIMARY KEY (blocklist_id, peer_id),
                FOREIGN KEY (blocklist_id) REFERENCES blocklist_subscriptions(id) ON DELETE CASCADE,
                FOREIGN KEY (peer_id) REFERENCES peers(id)
            )
            "#,
            [],
        )?;

        // Create redacted_posts table
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS redacted_posts (
                id TEXT PRIMARY KEY,
                thread_id TEXT NOT NULL,
                author_peer_id TEXT NOT NULL,
                parent_post_ids TEXT NOT NULL,
                known_child_ids TEXT,
                redaction_reason TEXT NOT NULL,
                discovered_at TEXT NOT NULL,
                FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE
            )
            "#,
            [],
        )?;

        // Create index for blocklist queries
        conn.execute(
            r#"
            CREATE INDEX IF NOT EXISTS idx_blocklist_entries_peer
            ON blocklist_entries(peer_id)
            "#,
            [],
        )?;

        Ok(())
    }
}
