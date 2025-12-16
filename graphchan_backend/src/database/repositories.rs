use super::models::{
    FileRecord, PeerRecord, PostRecord, ReactionRecord, ThreadRecord, ThreadMemberKey,
    DirectMessageRecord, ConversationRecord, BlockedPeerRecord, BlocklistSubscriptionRecord,
    BlocklistEntryRecord, RedactedPostRecord,
};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashMap;

pub trait ThreadRepository {
    fn create(&self, record: &ThreadRecord) -> Result<()>;
    fn upsert(&self, record: &ThreadRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<ThreadRecord>>;
    fn list_recent(&self, limit: usize) -> Result<Vec<ThreadRecord>>;
    fn set_rebroadcast(&self, thread_id: &str, rebroadcast: bool) -> Result<()>;
    fn should_rebroadcast(&self, thread_id: &str) -> Result<bool>;
    fn delete(&self, thread_id: &str) -> Result<()>;
    fn set_ignored(&self, thread_id: &str, ignored: bool) -> Result<()>;
    fn is_ignored(&self, thread_id: &str) -> Result<bool>;
}

pub trait PostRepository {
    fn create(&self, record: &PostRecord) -> Result<()>;
    fn upsert(&self, record: &PostRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<PostRecord>>;
    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<PostRecord>>;
    fn add_relationships(&self, child_id: &str, parent_ids: &[String]) -> Result<()>;
    fn parents_of(&self, child_id: &str) -> Result<Vec<String>>;
}

pub trait PeerRepository {
    fn upsert(&self, record: &PeerRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<PeerRecord>>;
    fn list(&self) -> Result<Vec<PeerRecord>>;
    fn delete(&self, id: &str) -> Result<()>;
}

pub trait FileRepository {
    fn attach(&self, record: &FileRecord) -> Result<()>;
    fn upsert(&self, record: &FileRecord) -> Result<()>;
    fn list_for_post(&self, post_id: &str) -> Result<Vec<FileRecord>>;
    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<FileRecord>>;
    fn get(&self, id: &str) -> Result<Option<FileRecord>>;
}

pub trait ReactionRepository {
    fn add(&self, record: &ReactionRecord) -> Result<()>;
    fn remove(&self, post_id: &str, reactor_peer_id: &str, emoji: &str) -> Result<()>;
    fn list_for_post(&self, post_id: &str) -> Result<Vec<ReactionRecord>>;
    /// Returns HashMap<emoji, count>
    fn count_for_post(&self, post_id: &str) -> Result<HashMap<String, usize>>;
}

pub trait ThreadMemberKeyRepository {
    fn add(&self, record: &ThreadMemberKey) -> Result<()>;
    fn get(&self, thread_id: &str, member_peer_id: &str) -> Result<Option<ThreadMemberKey>>;
    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<ThreadMemberKey>>;
    fn remove(&self, thread_id: &str, member_peer_id: &str) -> Result<()>;
}

pub trait DirectMessageRepository {
    fn create(&self, record: &DirectMessageRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<DirectMessageRecord>>;
    fn list_for_conversation(&self, conversation_id: &str, limit: usize) -> Result<Vec<DirectMessageRecord>>;
    fn mark_as_read(&self, id: &str, read_at: &str) -> Result<()>;
    fn count_unread(&self, to_peer_id: &str) -> Result<usize>;
}

pub trait ConversationRepository {
    fn upsert(&self, record: &ConversationRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<ConversationRecord>>;
    fn list(&self) -> Result<Vec<ConversationRecord>>;
    fn update_unread_count(&self, conversation_id: &str, count: i64) -> Result<()>;
    fn update_last_message(&self, conversation_id: &str, message_at: &str, preview: &str) -> Result<()>;
}

pub trait BlockedPeerRepository {
    fn block(&self, record: &BlockedPeerRecord) -> Result<()>;
    fn unblock(&self, peer_id: &str) -> Result<()>;
    fn is_blocked(&self, peer_id: &str) -> Result<bool>;
    fn list(&self) -> Result<Vec<BlockedPeerRecord>>;
}

pub trait BlocklistRepository {
    fn subscribe(&self, record: &BlocklistSubscriptionRecord) -> Result<()>;
    fn unsubscribe(&self, blocklist_id: &str) -> Result<()>;
    fn list_subscriptions(&self) -> Result<Vec<BlocklistSubscriptionRecord>>;
    fn add_entry(&self, entry: &BlocklistEntryRecord) -> Result<()>;
    fn remove_entry(&self, blocklist_id: &str, peer_id: &str) -> Result<()>;
    fn list_entries(&self, blocklist_id: &str) -> Result<Vec<BlocklistEntryRecord>>;
    fn is_in_any_blocklist(&self, peer_id: &str) -> Result<bool>;
}

pub trait RedactedPostRepository {
    fn create(&self, record: &RedactedPostRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<RedactedPostRecord>>;
    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<RedactedPostRecord>>;
}

/// Thin wrapper that will eventually host rusqlite-backed implementations.
pub struct SqliteRepositories<'conn> {
    conn: &'conn Connection,
}

impl<'conn> SqliteRepositories<'conn> {
    pub fn new(conn: &'conn Connection) -> Self {
        Self { conn }
    }

    pub fn threads(&self) -> impl ThreadRepository + '_ {
        SqliteThreadRepository { conn: self.conn }
    }

    pub fn posts(&self) -> impl PostRepository + '_ {
        SqlitePostRepository { conn: self.conn }
    }

    pub fn peers(&self) -> impl PeerRepository + '_ {
        SqlitePeerRepository { conn: self.conn }
    }

    pub fn files(&self) -> impl FileRepository + '_ {
        SqliteFileRepository { conn: self.conn }
    }

    pub fn reactions(&self) -> impl ReactionRepository + '_ {
        SqliteReactionRepository { conn: self.conn }
    }

    pub fn thread_member_keys(&self) -> impl ThreadMemberKeyRepository + '_ {
        SqliteThreadMemberKeyRepository { conn: self.conn }
    }

    pub fn direct_messages(&self) -> impl DirectMessageRepository + '_ {
        SqliteDirectMessageRepository { conn: self.conn }
    }

    pub fn conversations(&self) -> impl ConversationRepository + '_ {
        SqliteConversationRepository { conn: self.conn }
    }

    pub fn blocked_peers(&self) -> impl BlockedPeerRepository + '_ {
        SqliteBlockedPeerRepository { conn: self.conn }
    }

    pub fn blocklists(&self) -> impl BlocklistRepository + '_ {
        SqliteBlocklistRepository { conn: self.conn }
    }

    pub fn redacted_posts(&self) -> impl RedactedPostRepository + '_ {
        SqliteRedactedPostRepository { conn: self.conn }
    }

    pub fn conn(&self) -> &'conn Connection {
        self.conn
    }
}

struct SqliteThreadRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> ThreadRepository for SqliteThreadRepository<'conn> {
    fn create(&self, record: &ThreadRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO threads (id, title, creator_peer_id, created_at, pinned, thread_hash, visibility, topic_secret)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                record.id,
                record.title,
                record.creator_peer_id,
                record.created_at,
                if record.pinned { 1 } else { 0 },
                record.thread_hash,
                record.visibility,
                record.topic_secret
            ],
        )?;
        Ok(())
    }

    fn upsert(&self, record: &ThreadRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO threads (id, title, creator_peer_id, created_at, pinned, thread_hash, visibility, topic_secret)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                creator_peer_id = excluded.creator_peer_id,
                created_at = excluded.created_at,
                pinned = excluded.pinned,
                thread_hash = excluded.thread_hash,
                visibility = excluded.visibility,
                topic_secret = excluded.topic_secret
            "#,
            params![
                record.id,
                record.title,
                record.creator_peer_id,
                record.created_at,
                if record.pinned { 1 } else { 0 },
                record.thread_hash,
                record.visibility,
                record.topic_secret
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<ThreadRecord>> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT id, title, creator_peer_id, created_at, pinned, thread_hash,
                       COALESCE(visibility, 'social') as visibility, topic_secret
                FROM threads
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(ThreadRecord {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        creator_peer_id: row.get(2)?,
                        created_at: row.get(3)?,
                        pinned: row.get::<_, i64>(4)? != 0,
                        thread_hash: row.get(5)?,
                        visibility: row.get(6)?,
                        topic_secret: row.get(7)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn list_recent(&self, limit: usize) -> Result<Vec<ThreadRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, creator_peer_id, created_at, pinned, thread_hash,
                   COALESCE(visibility, 'social') as visibility, topic_secret
            FROM threads
            WHERE deleted = 0 AND ignored = 0
            ORDER BY datetime(created_at) DESC
            LIMIT ?1
            "#,
        )?;
        let rows = stmt.query_map(params![limit as i64], |row| {
            Ok(ThreadRecord {
                id: row.get(0)?,
                title: row.get(1)?,
                creator_peer_id: row.get(2)?,
                created_at: row.get(3)?,
                pinned: row.get::<_, i64>(4)? != 0,
                thread_hash: row.get(5)?,
                visibility: row.get(6)?,
                topic_secret: row.get(7)?,
            })
        })?;

        let mut threads = Vec::new();
        for row in rows {
            threads.push(row?);
        }
        Ok(threads)
    }

    fn set_rebroadcast(&self, thread_id: &str, rebroadcast: bool) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE threads
            SET rebroadcast = ?1
            WHERE id = ?2
            "#,
            params![if rebroadcast { 1 } else { 0 }, thread_id],
        )?;
        Ok(())
    }

    fn should_rebroadcast(&self, thread_id: &str) -> Result<bool> {
        let rebroadcast: i64 = self.conn.query_row(
            r#"
            SELECT rebroadcast
            FROM threads
            WHERE id = ?1
            "#,
            params![thread_id],
            |row| row.get(0),
        )?;
        Ok(rebroadcast != 0)
    }

    fn delete(&self, thread_id: &str) -> Result<()> {
        // Real DELETE - will cascade to posts, post_relationships, files, thread_tickets
        self.conn.execute(
            r#"
            DELETE FROM threads
            WHERE id = ?1
            "#,
            params![thread_id],
        )?;
        Ok(())
    }

    fn set_ignored(&self, thread_id: &str, ignored: bool) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE threads
            SET ignored = ?1
            WHERE id = ?2
            "#,
            params![if ignored { 1 } else { 0 }, thread_id],
        )?;
        Ok(())
    }

    fn is_ignored(&self, thread_id: &str) -> Result<bool> {
        let ignored: i64 = self.conn.query_row(
            r#"
            SELECT ignored
            FROM threads
            WHERE id = ?1
            "#,
            params![thread_id],
            |row| row.get(0),
        )?;
        Ok(ignored != 0)
    }
}

struct SqlitePostRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> PostRepository for SqlitePostRepository<'conn> {
    fn create(&self, record: &PostRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO posts (id, thread_id, author_peer_id, body, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                record.id,
                record.thread_id,
                record.author_peer_id,
                record.body,
                record.created_at,
                record.updated_at
            ],
        )?;
        Ok(())
    }

    fn upsert(&self, record: &PostRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO posts (id, thread_id, author_peer_id, body, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            ON CONFLICT(id) DO UPDATE SET
                thread_id = excluded.thread_id,
                author_peer_id = excluded.author_peer_id,
                body = excluded.body,
                created_at = excluded.created_at,
                updated_at = excluded.updated_at
            "#,
            params![
                record.id,
                record.thread_id,
                record.author_peer_id,
                record.body,
                record.created_at,
                record.updated_at
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<PostRecord>> {
        Ok(self
            .conn
            .query_row(
                r#"
                SELECT id, thread_id, author_peer_id, body, created_at, updated_at
                FROM posts
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(PostRecord {
                        id: row.get(0)?,
                        thread_id: row.get(1)?,
                        author_peer_id: row.get(2)?,
                        body: row.get(3)?,
                        created_at: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<PostRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, thread_id, author_peer_id, body, created_at, updated_at
            FROM posts
            WHERE thread_id = ?1
            ORDER BY datetime(created_at) ASC
            "#,
        )?;
        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(PostRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                author_peer_id: row.get(2)?,
                body: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;
        let mut posts = Vec::new();
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }

    fn add_relationships(&self, child_id: &str, parent_ids: &[String]) -> Result<()> {
        if parent_ids.is_empty() {
            return Ok(());
        }
        let tx = self.conn.unchecked_transaction()?;
        {
            let mut stmt = tx.prepare(
                r#"
                INSERT OR IGNORE INTO post_relationships (parent_id, child_id)
                VALUES (?1, ?2)
                "#,
            )?;
            for parent in parent_ids {
                stmt.execute(params![parent, child_id])?;
            }
        }
        tx.commit()?;
        Ok(())
    }

    fn parents_of(&self, child_id: &str) -> Result<Vec<String>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT parent_id
            FROM post_relationships
            WHERE child_id = ?1
            ORDER BY parent_id ASC
            "#,
        )?;
        let rows = stmt.query_map(params![child_id], |row| row.get::<_, String>(0))?;
        let mut parents = Vec::new();
        for row in rows {
            parents.push(row?);
        }
        Ok(parents)
    }
}

struct SqlitePeerRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> PeerRepository for SqlitePeerRepository<'conn> {
    fn upsert(&self, record: &PeerRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO peers (id, alias, friendcode, iroh_peer_id, gpg_fingerprint, x25519_pubkey, last_seen, trust_state, avatar_file_id, username, bio)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            ON CONFLICT(id) DO UPDATE SET
                alias = excluded.alias,
                friendcode = excluded.friendcode,
                iroh_peer_id = excluded.iroh_peer_id,
                gpg_fingerprint = excluded.gpg_fingerprint,
                x25519_pubkey = excluded.x25519_pubkey,
                last_seen = excluded.last_seen,
                trust_state = excluded.trust_state,
                avatar_file_id = excluded.avatar_file_id,
                username = excluded.username,
                bio = excluded.bio
            "#,
            params![
                record.id,
                record.alias,
                record.friendcode,
                record.iroh_peer_id,
                record.gpg_fingerprint,
                record.x25519_pubkey,
                record.last_seen,
                record.trust_state,
                record.avatar_file_id,
                record.username,
                record.bio
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<PeerRecord>> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT id, alias, friendcode, iroh_peer_id, gpg_fingerprint, x25519_pubkey, last_seen, trust_state, avatar_file_id, username, bio
                FROM peers
                WHERE id = ?1
                "#,
                params![id],
                |row| {
                    Ok(PeerRecord {
                        id: row.get(0)?,
                        alias: row.get(1)?,
                        friendcode: row.get(2)?,
                        iroh_peer_id: row.get(3)?,
                        gpg_fingerprint: row.get(4)?,
                        x25519_pubkey: row.get(5)?,
                        last_seen: row.get(6)?,
                        trust_state: row.get(7)?,
                        avatar_file_id: row.get(8)?,
                        username: row.get(9)?,
                        bio: row.get(10)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn list(&self) -> Result<Vec<PeerRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, alias, friendcode, iroh_peer_id, gpg_fingerprint, x25519_pubkey, last_seen, trust_state, avatar_file_id, username, bio
            FROM peers
            ORDER BY datetime(COALESCE(last_seen, '1970-01-01T00:00:00Z')) DESC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(PeerRecord {
                id: row.get(0)?,
                alias: row.get(1)?,
                friendcode: row.get(2)?,
                iroh_peer_id: row.get(3)?,
                gpg_fingerprint: row.get(4)?,
                x25519_pubkey: row.get(5)?,
                last_seen: row.get(6)?,
                trust_state: row.get(7)?,
                avatar_file_id: row.get(8)?,
                username: row.get(9)?,
                bio: row.get(10)?,
            })
        })?;
        let mut peers = Vec::new();
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
    }

    fn delete(&self, id: &str) -> Result<()> {
        self.conn.execute(
            "DELETE FROM peers WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }
}

struct SqliteFileRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> FileRepository for SqliteFileRepository<'conn> {
    fn attach(&self, record: &FileRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO files (id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
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
                record.ticket
            ],
        )?;
        Ok(())
    }

    fn upsert(&self, record: &FileRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO files (id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                post_id = excluded.post_id,
                path = excluded.path,
                original_name = excluded.original_name,
                mime = excluded.mime,
                blob_id = excluded.blob_id,
                size_bytes = excluded.size_bytes,
                checksum = excluded.checksum,
                ticket = excluded.ticket
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
                record.ticket
            ],
        )?;
        Ok(())
    }

    fn list_for_post(&self, post_id: &str) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket
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
            })
        })?;
        let mut files = Vec::new();
        for row in rows {
            files.push(row?);
        }
        Ok(files)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<FileRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT f.id, f.post_id, f.path, f.original_name, f.mime, f.blob_id, f.size_bytes, f.checksum, f.ticket
            FROM files f
            INNER JOIN posts p ON f.post_id = p.id
            WHERE p.thread_id = ?1
            ORDER BY f.id ASC
            "#,
        )?;
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
                SELECT id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket
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
                    })
                },
            )
            .optional()?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::MIGRATIONS;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATIONS).expect("migrations");
        conn
    }

    #[test]
    fn thread_and_post_repositories_work() {
        let conn = setup_conn();
        let repos = SqliteRepositories::new(&conn);

        let peer = PeerRecord {
            id: "peer-1".into(),
            alias: Some("author".into()),
            friendcode: None,
            iroh_peer_id: None,
            gpg_fingerprint: None,
            last_seen: None,
            trust_state: "unknown".into(),
            avatar_file_id: None,
        };
        repos.peers().upsert(&peer).unwrap();

        let thread = ThreadRecord {
            id: "thread-1".into(),
            title: "First".into(),
            creator_peer_id: Some(peer.id.clone()),
            created_at: "2024-01-01T00:00:00Z".into(),
            pinned: false,
            thread_hash: None,
        };
        repos.threads().create(&thread).unwrap();

        let fetched = repos.threads().get("thread-1").unwrap().unwrap();
        assert_eq!(fetched.title, "First");

        let post = PostRecord {
            id: "post-1".into(),
            thread_id: thread.id.clone(),
            author_peer_id: Some("peer-1".into()),
            body: "Hello".into(),
            created_at: "2024-01-01T00:00:01Z".into(),
            updated_at: None,
        };
        repos.posts().create(&post).unwrap();

        let posts = repos.posts().list_for_thread(&thread.id).unwrap();
        assert_eq!(posts.len(), 1);
        assert_eq!(posts[0].body, "Hello");
    }

    #[test]
    fn peer_and_file_repositories_work() {
        let conn = setup_conn();
        let repos = SqliteRepositories::new(&conn);

        let peer = PeerRecord {
            id: "peer-1".into(),
            alias: Some("alice".into()),
            friendcode: Some("friend-code".into()),
            iroh_peer_id: Some("peer-id".into()),
            gpg_fingerprint: Some("fingerprint".into()),
            last_seen: Some("2024-01-01T00:00:00Z".into()),
            trust_state: "trusted".into(),
            avatar_file_id: None,
        };
        repos.peers().upsert(&peer).unwrap();
        let fetched = repos.peers().get("peer-1").unwrap().unwrap();
        assert_eq!(fetched.alias.as_deref(), Some("alice"));

        let thread = ThreadRecord {
            id: "thread-1".into(),
            title: "Downloads".into(),
            creator_peer_id: Some(peer.id.clone()),
            created_at: "2024-01-01T00:00:00Z".into(),
            pinned: false,
            thread_hash: None,
        };
        repos.threads().create(&thread).unwrap();

        let post = PostRecord {
            id: "post-1".into(),
            thread_id: thread.id.clone(),
            author_peer_id: Some(peer.id.clone()),
            body: "Attachment".into(),
            created_at: "2024-01-01T00:01:00Z".into(),
            updated_at: None,
        };
        repos.posts().create(&post).unwrap();

        let file = FileRecord {
            id: "file-1".into(),
            post_id: post.id.clone(),
            path: "files/uploads/file-1.bin".into(),
            original_name: Some("file-1.bin".into()),
            mime: Some("application/octet-stream".into()),
            blob_id: Some("blob-1".into()),
            size_bytes: Some(42),
            checksum: Some("sha256:deadbeef".into()),
            ticket: None,
        };
        repos.files().attach(&file).unwrap();
        let files = repos.files().list_for_post(&post.id).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].checksum.as_deref(), Some("sha256:deadbeef"));
    }
}

struct SqliteReactionRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> ReactionRepository for SqliteReactionRepository<'conn> {
    fn add(&self, record: &ReactionRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO reactions (post_id, reactor_peer_id, emoji, signature, created_at)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(post_id, reactor_peer_id, emoji) DO UPDATE SET
                signature = excluded.signature,
                created_at = excluded.created_at
            "#,
            params![
                record.post_id,
                record.reactor_peer_id,
                record.emoji,
                record.signature,
                record.created_at,
            ],
        )?;
        Ok(())
    }

    fn remove(&self, post_id: &str, reactor_peer_id: &str, emoji: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM reactions
            WHERE post_id = ?1 AND reactor_peer_id = ?2 AND emoji = ?3
            "#,
            params![post_id, reactor_peer_id, emoji],
        )?;
        Ok(())
    }

    fn list_for_post(&self, post_id: &str) -> Result<Vec<ReactionRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT post_id, reactor_peer_id, emoji, signature, created_at
            FROM reactions
            WHERE post_id = ?1
            ORDER BY created_at ASC
            "#,
        )?;
        let rows = stmt.query_map(params![post_id], |row| {
            Ok(ReactionRecord {
                post_id: row.get(0)?,
                reactor_peer_id: row.get(1)?,
                emoji: row.get(2)?,
                signature: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut reactions = Vec::new();
        for row in rows {
            reactions.push(row?);
        }
        Ok(reactions)
    }

    fn count_for_post(&self, post_id: &str) -> Result<HashMap<String, usize>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT emoji, COUNT(*) as count
            FROM reactions
            WHERE post_id = ?1
            GROUP BY emoji
            "#,
        )?;
        let rows = stmt.query_map(params![post_id], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;

        let mut counts = HashMap::new();
        for row in rows {
            let (emoji, count) = row?;
            counts.insert(emoji, count);
        }
        Ok(counts)
    }
}

struct SqliteThreadMemberKeyRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> ThreadMemberKeyRepository for SqliteThreadMemberKeyRepository<'conn> {
    fn add(&self, record: &ThreadMemberKey) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO thread_member_keys (thread_id, member_peer_id, wrapped_key_ciphertext, wrapped_key_nonce)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![
                record.thread_id,
                record.member_peer_id,
                record.wrapped_key_ciphertext,
                record.wrapped_key_nonce
            ],
        )?;
        Ok(())
    }

    fn get(&self, thread_id: &str, member_peer_id: &str) -> Result<Option<ThreadMemberKey>> {
        let result = self.conn.query_row(
            r#"
            SELECT thread_id, member_peer_id, wrapped_key_ciphertext, wrapped_key_nonce
            FROM thread_member_keys
            WHERE thread_id = ?1 AND member_peer_id = ?2
            "#,
            params![thread_id, member_peer_id],
            |row| {
                Ok(ThreadMemberKey {
                    thread_id: row.get(0)?,
                    member_peer_id: row.get(1)?,
                    wrapped_key_ciphertext: row.get(2)?,
                    wrapped_key_nonce: row.get(3)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<ThreadMemberKey>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT thread_id, member_peer_id, wrapped_key_ciphertext, wrapped_key_nonce
            FROM thread_member_keys
            WHERE thread_id = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(ThreadMemberKey {
                thread_id: row.get(0)?,
                member_peer_id: row.get(1)?,
                wrapped_key_ciphertext: row.get(2)?,
                wrapped_key_nonce: row.get(3)?,
            })
        })?;

        let mut keys = Vec::new();
        for row in rows {
            keys.push(row?);
        }
        Ok(keys)
    }

    fn remove(&self, thread_id: &str, member_peer_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM thread_member_keys
            WHERE thread_id = ?1 AND member_peer_id = ?2
            "#,
            params![thread_id, member_peer_id],
        )?;
        Ok(())
    }
}

struct SqliteDirectMessageRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> DirectMessageRepository for SqliteDirectMessageRepository<'conn> {
    fn create(&self, record: &DirectMessageRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO direct_messages (id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                record.id,
                record.conversation_id,
                record.from_peer_id,
                record.to_peer_id,
                record.encrypted_body,
                record.nonce,
                record.created_at,
                record.read_at
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<DirectMessageRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at
            FROM direct_messages
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(DirectMessageRecord {
                    id: row.get(0)?,
                    conversation_id: row.get(1)?,
                    from_peer_id: row.get(2)?,
                    to_peer_id: row.get(3)?,
                    encrypted_body: row.get(4)?,
                    nonce: row.get(5)?,
                    created_at: row.get(6)?,
                    read_at: row.get(7)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list_for_conversation(&self, conversation_id: &str, limit: usize) -> Result<Vec<DirectMessageRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, conversation_id, from_peer_id, to_peer_id, encrypted_body, nonce, created_at, read_at
            FROM direct_messages
            WHERE conversation_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
        )?;

        let rows = stmt.query_map(params![conversation_id, limit as i64], |row| {
            Ok(DirectMessageRecord {
                id: row.get(0)?,
                conversation_id: row.get(1)?,
                from_peer_id: row.get(2)?,
                to_peer_id: row.get(3)?,
                encrypted_body: row.get(4)?,
                nonce: row.get(5)?,
                created_at: row.get(6)?,
                read_at: row.get(7)?,
            })
        })?;

        let mut messages = Vec::new();
        for row in rows {
            messages.push(row?);
        }
        Ok(messages)
    }

    fn mark_as_read(&self, id: &str, read_at: &str) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE direct_messages
            SET read_at = ?1
            WHERE id = ?2
            "#,
            params![read_at, id],
        )?;
        Ok(())
    }

    fn count_unread(&self, to_peer_id: &str) -> Result<usize> {
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM direct_messages
            WHERE to_peer_id = ?1 AND read_at IS NULL
            "#,
            params![to_peer_id],
            |row| row.get(0),
        )?;
        Ok(count as usize)
    }
}

struct SqliteConversationRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> ConversationRepository for SqliteConversationRepository<'conn> {
    fn upsert(&self, record: &ConversationRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO conversations (id, peer_id, last_message_at, last_message_preview, unread_count)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                last_message_at = excluded.last_message_at,
                last_message_preview = excluded.last_message_preview,
                unread_count = excluded.unread_count
            "#,
            params![
                record.id,
                record.peer_id,
                record.last_message_at,
                record.last_message_preview,
                record.unread_count
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<ConversationRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, peer_id, last_message_at, last_message_preview, unread_count
            FROM conversations
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(ConversationRecord {
                    id: row.get(0)?,
                    peer_id: row.get(1)?,
                    last_message_at: row.get(2)?,
                    last_message_preview: row.get(3)?,
                    unread_count: row.get(4)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list(&self) -> Result<Vec<ConversationRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, peer_id, last_message_at, last_message_preview, unread_count
            FROM conversations
            ORDER BY last_message_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(ConversationRecord {
                id: row.get(0)?,
                peer_id: row.get(1)?,
                last_message_at: row.get(2)?,
                last_message_preview: row.get(3)?,
                unread_count: row.get(4)?,
            })
        })?;

        let mut conversations = Vec::new();
        for row in rows {
            conversations.push(row?);
        }
        Ok(conversations)
    }

    fn update_unread_count(&self, conversation_id: &str, count: i64) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE conversations
            SET unread_count = ?1
            WHERE id = ?2
            "#,
            params![count, conversation_id],
        )?;
        Ok(())
    }

    fn update_last_message(&self, conversation_id: &str, message_at: &str, preview: &str) -> Result<()> {
        self.conn.execute(
            r#"
            UPDATE conversations
            SET last_message_at = ?1, last_message_preview = ?2
            WHERE id = ?3
            "#,
            params![message_at, preview, conversation_id],
        )?;
        Ok(())
    }
}

struct SqliteBlockedPeerRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> BlockedPeerRepository for SqliteBlockedPeerRepository<'conn> {
    fn block(&self, record: &BlockedPeerRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO blocked_peers (peer_id, reason, blocked_at)
            VALUES (?1, ?2, ?3)
            "#,
            params![record.peer_id, record.reason, record.blocked_at],
        )?;
        Ok(())
    }

    fn unblock(&self, peer_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM blocked_peers
            WHERE peer_id = ?1
            "#,
            params![peer_id],
        )?;
        Ok(())
    }

    fn is_blocked(&self, peer_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM blocked_peers
            WHERE peer_id = ?1
            "#,
            params![peer_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn list(&self) -> Result<Vec<BlockedPeerRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT peer_id, reason, blocked_at
            FROM blocked_peers
            ORDER BY blocked_at DESC
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BlockedPeerRecord {
                peer_id: row.get(0)?,
                reason: row.get(1)?,
                blocked_at: row.get(2)?,
            })
        })?;

        let mut peers = Vec::new();
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
    }
}

struct SqliteBlocklistRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> BlocklistRepository for SqliteBlocklistRepository<'conn> {
    fn subscribe(&self, record: &BlocklistSubscriptionRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO blocklist_subscriptions
            (id, maintainer_peer_id, name, description, auto_apply, last_synced_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![
                record.id,
                record.maintainer_peer_id,
                record.name,
                record.description,
                if record.auto_apply { 1 } else { 0 },
                record.last_synced_at
            ],
        )?;
        Ok(())
    }

    fn unsubscribe(&self, blocklist_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM blocklist_subscriptions
            WHERE id = ?1
            "#,
            params![blocklist_id],
        )?;
        Ok(())
    }

    fn list_subscriptions(&self) -> Result<Vec<BlocklistSubscriptionRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, maintainer_peer_id, name, description, auto_apply, last_synced_at
            FROM blocklist_subscriptions
            ORDER BY name
            "#,
        )?;

        let rows = stmt.query_map([], |row| {
            Ok(BlocklistSubscriptionRecord {
                id: row.get(0)?,
                maintainer_peer_id: row.get(1)?,
                name: row.get(2)?,
                description: row.get(3)?,
                auto_apply: row.get::<_, i64>(4)? != 0,
                last_synced_at: row.get(5)?,
            })
        })?;

        let mut lists = Vec::new();
        for row in rows {
            lists.push(row?);
        }
        Ok(lists)
    }

    fn add_entry(&self, entry: &BlocklistEntryRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO blocklist_entries (blocklist_id, peer_id, reason, added_at)
            VALUES (?1, ?2, ?3, ?4)
            "#,
            params![entry.blocklist_id, entry.peer_id, entry.reason, entry.added_at],
        )?;
        Ok(())
    }

    fn remove_entry(&self, blocklist_id: &str, peer_id: &str) -> Result<()> {
        self.conn.execute(
            r#"
            DELETE FROM blocklist_entries
            WHERE blocklist_id = ?1 AND peer_id = ?2
            "#,
            params![blocklist_id, peer_id],
        )?;
        Ok(())
    }

    fn list_entries(&self, blocklist_id: &str) -> Result<Vec<BlocklistEntryRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT blocklist_id, peer_id, reason, added_at
            FROM blocklist_entries
            WHERE blocklist_id = ?1
            ORDER BY added_at DESC
            "#,
        )?;

        let rows = stmt.query_map(params![blocklist_id], |row| {
            Ok(BlocklistEntryRecord {
                blocklist_id: row.get(0)?,
                peer_id: row.get(1)?,
                reason: row.get(2)?,
                added_at: row.get(3)?,
            })
        })?;

        let mut entries = Vec::new();
        for row in rows {
            entries.push(row?);
        }
        Ok(entries)
    }

    fn is_in_any_blocklist(&self, peer_id: &str) -> Result<bool> {
        let count: i64 = self.conn.query_row(
            r#"
            SELECT COUNT(*)
            FROM blocklist_entries e
            JOIN blocklist_subscriptions s ON e.blocklist_id = s.id
            WHERE e.peer_id = ?1 AND s.auto_apply = 1
            "#,
            params![peer_id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }
}

struct SqliteRedactedPostRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> RedactedPostRepository for SqliteRedactedPostRepository<'conn> {
    fn create(&self, record: &RedactedPostRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT OR REPLACE INTO redacted_posts
            (id, thread_id, author_peer_id, parent_post_ids, known_child_ids, redaction_reason, discovered_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            "#,
            params![
                record.id,
                record.thread_id,
                record.author_peer_id,
                record.parent_post_ids,
                record.known_child_ids,
                record.redaction_reason,
                record.discovered_at
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<RedactedPostRecord>> {
        let result = self.conn.query_row(
            r#"
            SELECT id, thread_id, author_peer_id, parent_post_ids, known_child_ids, redaction_reason, discovered_at
            FROM redacted_posts
            WHERE id = ?1
            "#,
            params![id],
            |row| {
                Ok(RedactedPostRecord {
                    id: row.get(0)?,
                    thread_id: row.get(1)?,
                    author_peer_id: row.get(2)?,
                    parent_post_ids: row.get(3)?,
                    known_child_ids: row.get(4)?,
                    redaction_reason: row.get(5)?,
                    discovered_at: row.get(6)?,
                })
            },
        ).optional()?;
        Ok(result)
    }

    fn list_for_thread(&self, thread_id: &str) -> Result<Vec<RedactedPostRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, thread_id, author_peer_id, parent_post_ids, known_child_ids, redaction_reason, discovered_at
            FROM redacted_posts
            WHERE thread_id = ?1
            ORDER BY discovered_at
            "#,
        )?;

        let rows = stmt.query_map(params![thread_id], |row| {
            Ok(RedactedPostRecord {
                id: row.get(0)?,
                thread_id: row.get(1)?,
                author_peer_id: row.get(2)?,
                parent_post_ids: row.get(3)?,
                known_child_ids: row.get(4)?,
                redaction_reason: row.get(5)?,
                discovered_at: row.get(6)?,
            })
        })?;

        let mut posts = Vec::new();
        for row in rows {
            posts.push(row?);
        }
        Ok(posts)
    }
}
