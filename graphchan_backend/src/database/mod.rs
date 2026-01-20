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
        author_friendcode TEXT,
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

    -- Migration: Add author_friendcode to posts table (now in base schema)
    -- ALTER TABLE posts ADD COLUMN author_friendcode TEXT;
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
            self.ensure_peer_profile_columns(conn)?;
            self.ensure_thread_blob_ticket_column(conn)?;
            self.ensure_thread_hash_column(conn)?;
            self.ensure_x25519_pubkey_column(conn)?;
            self.ensure_thread_visibility_columns(conn)?;
            self.ensure_thread_sync_status_column(conn)?;
            self.ensure_thread_member_keys_table(conn)?;
            self.ensure_dm_tables(conn)?;
            self.ensure_blocking_tables(conn)?;
            self.ensure_ip_blocking_tables(conn)?;
            self.ensure_fts5_search_tables(conn)?;
            self.ensure_file_download_status_column(conn)?;
            self.ensure_post_metadata_column(conn)?;
            self.ensure_peers_agents_column(conn)?;
            self.ensure_topic_tables(conn)?;
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

    pub fn with_conn<T, F>(&self, f: F) -> Result<T>
    where
        F: FnOnce(&Connection) -> Result<T>,
    {
        let guard = self
            .conn
            .lock()
            .map_err(|_| anyhow!("database mutex poisoned"))?;
        f(&guard)
    }

    /// Get a setting value by key
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT value FROM settings WHERE key = ?1",
                [key],
                |row| row.get::<_, String>(0),
            )
            .optional()
            .context("failed to query setting")
        })
    }

    /// Set a setting value (upsert)
    pub fn set_setting(&self, key: &str, value: &str) -> Result<()> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO settings (key, value) VALUES (?1, ?2)
                 ON CONFLICT(key) DO UPDATE SET value = ?2",
                [key, value],
            )
            .context("failed to set setting")?;
            Ok(())
        })
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

    fn ensure_peer_profile_columns(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(peers)")?;
        let mut has_username = false;
        let mut has_bio = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("username") {
                has_username = true;
            }
            if name.eq_ignore_ascii_case("bio") {
                has_bio = true;
            }
        }
        if !has_username {
            conn.execute("ALTER TABLE peers ADD COLUMN username TEXT", [])?;
        }
        if !has_bio {
            conn.execute("ALTER TABLE peers ADD COLUMN bio TEXT", [])?;
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

    fn ensure_thread_sync_status_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(threads)")?;
        let mut has_sync_status = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("sync_status") {
                has_sync_status = true;
                break;
            }
        }
        if !has_sync_status {
            // Default to 'downloaded' for existing threads (they're already in DB)
            // Values: 'announced', 'downloading', 'downloaded', 'failed'
            conn.execute("ALTER TABLE threads ADD COLUMN sync_status TEXT DEFAULT 'downloaded'", [])?;
        }
        Ok(())
    }

    fn ensure_file_download_status_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(files)")?;
        let mut has_download_status = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("download_status") {
                has_download_status = true;
                break;
            }
        }
        if !has_download_status {
            // Default to 'available' for existing files (they're already in DB and physically present)
            // Values: 'pending', 'downloading', 'available', 'failed'
            conn.execute("ALTER TABLE files ADD COLUMN download_status TEXT DEFAULT 'available'", [])?;
        }
        Ok(())
    }

    fn ensure_post_metadata_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(posts)")?;
        let mut has_metadata = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("metadata") {
                has_metadata = true;
                break;
            }
        }
        if !has_metadata {
            // JSON-encoded PostMetadata (agent info, client info, etc.)
            conn.execute("ALTER TABLE posts ADD COLUMN metadata TEXT", [])?;
        }
        Ok(())
    }

    fn ensure_peers_agents_column(&self, conn: &Connection) -> Result<()> {
        let mut stmt = conn.prepare("PRAGMA table_info(peers)")?;
        let mut has_agents = false;
        let rows = stmt.query_map([], |row| {
            let name: String = row.get(1)?;
            Ok(name)
        })?;
        for row in rows {
            let name = row?;
            if name.eq_ignore_ascii_case("agents") {
                has_agents = true;
                break;
            }
        }
        if !has_agents {
            // JSON-encoded Vec<String> of authorized agent names
            conn.execute("ALTER TABLE peers ADD COLUMN agents TEXT", [])?;
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

    fn ensure_fts5_search_tables(&self, conn: &Connection) -> Result<()> {
        // Drop old triggers and tables if they exist (for migration safety)
        conn.execute("DROP TRIGGER IF EXISTS posts_fts_insert", [])?;
        conn.execute("DROP TRIGGER IF EXISTS posts_fts_update", [])?;
        conn.execute("DROP TRIGGER IF EXISTS posts_fts_delete", [])?;
        conn.execute("DROP TRIGGER IF EXISTS files_fts_insert", [])?;
        conn.execute("DROP TRIGGER IF EXISTS files_fts_delete", [])?;
        conn.execute("DROP TABLE IF EXISTS posts_fts", [])?;
        conn.execute("DROP TABLE IF EXISTS files_fts", [])?;

        // Ensure FTS5 internal tables are also cleaned up
        conn.execute("VACUUM", [])?;

        // Posts FTS5 table
        conn.execute(
            r#"CREATE VIRTUAL TABLE posts_fts USING fts5(
                id UNINDEXED,
                thread_id UNINDEXED,
                body,
                content='posts',
                content_rowid='rowid',
                tokenize='porter unicode61'
            )"#,
            [],
        )?;

        // Files FTS5 table
        conn.execute(
            r#"CREATE VIRTUAL TABLE files_fts USING fts5(
                id UNINDEXED,
                post_id UNINDEXED,
                original_name,
                path,
                content='files',
                content_rowid='rowid',
                tokenize='porter unicode61'
            )"#,
            [],
        )?;

        // Populate existing posts
        conn.execute(
            "INSERT INTO posts_fts(rowid, id, thread_id, body)
             SELECT rowid, id, thread_id, body FROM posts
             WHERE rowid NOT IN (SELECT rowid FROM posts_fts)",
            [],
        )?;

        // Populate existing files (join with posts to get thread_id)
        conn.execute(
            "INSERT INTO files_fts(rowid, id, post_id, original_name, path)
             SELECT f.rowid, f.id, f.post_id, f.original_name, f.path
             FROM files f
             WHERE f.rowid NOT IN (SELECT rowid FROM files_fts)",
            [],
        )?;

        // Triggers for posts
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS posts_fts_insert
             AFTER INSERT ON posts BEGIN
                 INSERT INTO posts_fts(rowid, id, thread_id, body)
                 VALUES (new.rowid, new.id, new.thread_id, new.body);
             END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS posts_fts_update
             AFTER UPDATE ON posts BEGIN
                 UPDATE posts_fts SET body = new.body WHERE rowid = old.rowid;
             END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS posts_fts_delete
             AFTER DELETE ON posts BEGIN
                 DELETE FROM posts_fts WHERE rowid = old.rowid;
             END",
            [],
        )?;

        // Triggers for files
        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS files_fts_insert
             AFTER INSERT ON files BEGIN
                 INSERT INTO files_fts(rowid, id, post_id, original_name, path)
                 VALUES (new.rowid, new.id, new.post_id, new.original_name, new.path);
             END",
            [],
        )?;

        conn.execute(
            "CREATE TRIGGER IF NOT EXISTS files_fts_delete
             AFTER DELETE ON files BEGIN
                 DELETE FROM files_fts WHERE rowid = old.rowid;
             END",
            [],
        )?;

        Ok(())
    }

    fn ensure_ip_blocking_tables(&self, conn: &Connection) -> Result<()> {
        // Create peer_ips table for tracking peer IP addresses
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS peer_ips (
                peer_id TEXT NOT NULL,
                ip_address TEXT NOT NULL,
                last_seen INTEGER NOT NULL,
                PRIMARY KEY (peer_id, ip_address),
                FOREIGN KEY (peer_id) REFERENCES peers(id)
            )
            "#,
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_peer_ips_ip ON peer_ips(ip_address)",
            [],
        )?;

        // Create ip_blocks table for user's IP blocking preferences
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS ip_blocks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                ip_or_range TEXT NOT NULL,
                block_type TEXT NOT NULL,
                blocked_at INTEGER NOT NULL,
                reason TEXT,
                active INTEGER NOT NULL DEFAULT 1,
                hit_count INTEGER DEFAULT 0
            )
            "#,
            [],
        )?;

        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_ip_blocks_active ON ip_blocks(active)",
            [],
        )?;

        Ok(())
    }

    fn ensure_topic_tables(&self, conn: &Connection) -> Result<()> {
        // Create user_topics table - tracks which topics the user subscribes to
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS user_topics (
                topic_id TEXT PRIMARY KEY,
                subscribed_at TEXT NOT NULL
            )
            "#,
            [],
        )?;

        // Create thread_topics table - many-to-many relationship between threads and topics
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS thread_topics (
                thread_id TEXT NOT NULL,
                topic_id TEXT NOT NULL,
                PRIMARY KEY (thread_id, topic_id),
                FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE
            )
            "#,
            [],
        )?;

        // Create index for querying threads by topic
        conn.execute(
            "CREATE INDEX IF NOT EXISTS idx_thread_topics_topic ON thread_topics(topic_id)",
            [],
        )?;

        Ok(())
    }
}
