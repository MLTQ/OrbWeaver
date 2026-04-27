pub mod models;
pub mod repositories;

use crate::config::GraphchanPaths;
use anyhow::{Context, Result};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use std::sync::Arc;

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

/// Versioned schema migrations. Each entry is run in order and recorded in
/// the `schema_migrations` table; a migration with a given version is run at
/// most once per database. New schema changes should append a new entry here
/// rather than spawning another `ensure_*` helper.
///
/// Existing entries must NEVER be edited — they may have run on databases in
/// the wild. To fix a botched migration, append a new one that repairs the
/// state.
const VERSIONED_MIGRATIONS: &[(i64, &str)] = &[
    // version 1: marker so future migrations have somewhere to land. The
    // base schema is still defined in MIGRATIONS above for first-run
    // bootstrapping; any column or table added after the initial release
    // belongs in a new entry here.
    (
        1,
        "-- baseline (no-op; pre-existing schema lives in MIGRATIONS)",
    ),
    // version 2: decrypt_status on direct_messages.
    // Values: 'decrypted' (default — message was successfully decrypted on
    // receipt), 'pending_key' (sender's x25519 key was not yet known —
    // retry when their profile arrives), 'failed' (decryption error
    // unrelated to key availability — corruption / wrong recipient).
    // Existing rows default to 'decrypted' since they all decrypted before
    // this column existed.
    (
        2,
        "ALTER TABLE direct_messages ADD COLUMN decrypt_status TEXT DEFAULT 'decrypted';",
    ),
];

/// Idempotent column-add helper. Probes the table's columns and runs
/// `ALTER TABLE ... ADD COLUMN ...` only if the column is missing. Used by
/// the legacy `ensure_*` helpers to keep them defensive and short.
fn add_column_if_missing(
    conn: &Connection,
    table: &str,
    column: &str,
    type_and_default: &str,
) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let already_present = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .filter_map(|r| r.ok())
        .any(|name| name.eq_ignore_ascii_case(column));
    drop(stmt);
    if !already_present {
        conn.execute(
            &format!("ALTER TABLE {table} ADD COLUMN {column} {type_and_default}"),
            [],
        )?;
    }
    Ok(())
}

fn run_versioned_migrations(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL
        )
        "#,
    )?;

    let current: i64 = conn
        .query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |row| row.get(0),
        )
        .context("read schema_migrations max version")?;

    for (version, sql) in VERSIONED_MIGRATIONS {
        if *version <= current {
            continue;
        }
        tracing::info!(version, "applying schema migration");
        conn.execute_batch(sql)
            .with_context(|| format!("schema migration {version} failed"))?;
        conn.execute(
            "INSERT INTO schema_migrations (version, applied_at) VALUES (?1, ?2)",
            params![version, crate::utils::now_utc_iso()],
        )?;
    }
    Ok(())
}

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
            // Legacy ensure_* helpers run BEFORE versioned migrations because
            // some of them (ensure_dm_tables, ensure_blocking_tables, etc.)
            // CREATE TABLE IF NOT EXISTS, and v2+ migrations may ALTER those
            // tables. On a fresh DB the legacy helpers materialize the schema
            // first, then the versioned migrations evolve it.
            // Each helper is defensively idempotent so re-running is a no-op.
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
            self.ensure_import_tracking(conn)?;
            run_versioned_migrations(conn)?;
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
        add_column_if_missing(conn, "node_identity", "friendcode", "TEXT")
    }

    fn ensure_files_schema_locked(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "files", "original_name", "TEXT")?;
        add_column_if_missing(conn, "files", "ticket", "TEXT")
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
        let guard = self.conn.lock();
        f(&guard)
    }

    /// Get a setting value by key
    pub fn get_setting(&self, key: &str) -> Result<Option<String>> {
        self.with_conn(|conn| {
            conn.query_row("SELECT value FROM settings WHERE key = ?1", [key], |row| {
                row.get::<_, String>(0)
            })
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
        add_column_if_missing(conn, "peers", "avatar_file_id", "TEXT")
    }

    fn ensure_peer_profile_columns(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "peers", "username", "TEXT")?;
        add_column_if_missing(conn, "peers", "bio", "TEXT")
    }

    fn ensure_thread_blob_ticket_column(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "threads", "blob_ticket", "TEXT")
    }

    fn ensure_thread_hash_column(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "threads", "thread_hash", "TEXT")
    }

    fn ensure_x25519_pubkey_column(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "peers", "x25519_pubkey", "TEXT")
    }

    fn ensure_thread_visibility_columns(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "threads", "visibility", "TEXT DEFAULT 'social'")?;
        add_column_if_missing(conn, "threads", "topic_secret", "TEXT")
    }

    fn ensure_thread_sync_status_column(&self, conn: &Connection) -> Result<()> {
        // Values: 'announced', 'downloading', 'downloaded', 'failed'.
        // Default 'downloaded' so existing rows are treated as already in sync.
        add_column_if_missing(conn, "threads", "sync_status", "TEXT DEFAULT 'downloaded'")
    }

    fn ensure_file_download_status_column(&self, conn: &Connection) -> Result<()> {
        // Values: 'pending', 'downloading', 'available', 'failed'.
        add_column_if_missing(conn, "files", "download_status", "TEXT DEFAULT 'available'")
    }

    fn ensure_post_metadata_column(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "posts", "metadata", "TEXT")
    }

    fn ensure_peers_agents_column(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "peers", "agents", "TEXT")
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

    fn ensure_import_tracking(&self, conn: &Connection) -> Result<()> {
        add_column_if_missing(conn, "threads", "source_url", "TEXT")?;
        add_column_if_missing(conn, "threads", "source_platform", "TEXT")?;
        add_column_if_missing(conn, "threads", "last_refreshed_at", "TEXT")?;

        // Create import_post_map table for dedup during refresh
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS import_post_map (
                thread_id TEXT NOT NULL,
                external_id TEXT NOT NULL,
                internal_id TEXT NOT NULL,
                PRIMARY KEY (thread_id, external_id),
                FOREIGN KEY (thread_id) REFERENCES threads(id) ON DELETE CASCADE,
                FOREIGN KEY (internal_id) REFERENCES posts(id) ON DELETE CASCADE
            )
            "#,
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
