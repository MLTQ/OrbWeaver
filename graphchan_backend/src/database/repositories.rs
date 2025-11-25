use super::models::{FileRecord, PeerRecord, PostRecord, ThreadRecord};
use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};

pub trait ThreadRepository {
    fn create(&self, record: &ThreadRecord) -> Result<()>;
    fn upsert(&self, record: &ThreadRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<ThreadRecord>>;
    fn list_recent(&self, limit: usize) -> Result<Vec<ThreadRecord>>;
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
}

pub trait FileRepository {
    fn attach(&self, record: &FileRecord) -> Result<()>;
    fn upsert(&self, record: &FileRecord) -> Result<()>;
    fn list_for_post(&self, post_id: &str) -> Result<Vec<FileRecord>>;
    fn get(&self, id: &str) -> Result<Option<FileRecord>>;
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
}

struct SqliteThreadRepository<'conn> {
    conn: &'conn Connection,
}

impl<'conn> ThreadRepository for SqliteThreadRepository<'conn> {
    fn create(&self, record: &ThreadRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO threads (id, title, creator_peer_id, created_at, pinned)
            VALUES (?1, ?2, ?3, ?4, ?5)
            "#,
            params![
                record.id,
                record.title,
                record.creator_peer_id,
                record.created_at,
                if record.pinned { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }

    fn upsert(&self, record: &ThreadRecord) -> Result<()> {
        self.conn.execute(
            r#"
            INSERT INTO threads (id, title, creator_peer_id, created_at, pinned)
            VALUES (?1, ?2, ?3, ?4, ?5)
            ON CONFLICT(id) DO UPDATE SET
                title = excluded.title,
                creator_peer_id = excluded.creator_peer_id,
                created_at = excluded.created_at,
                pinned = excluded.pinned
            "#,
            params![
                record.id,
                record.title,
                record.creator_peer_id,
                record.created_at,
                if record.pinned { 1 } else { 0 }
            ],
        )?;
        Ok(())
    }

    fn get(&self, id: &str) -> Result<Option<ThreadRecord>> {
        let row = self
            .conn
            .query_row(
                r#"
                SELECT id, title, creator_peer_id, created_at, pinned
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
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn list_recent(&self, limit: usize) -> Result<Vec<ThreadRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, title, creator_peer_id, created_at, pinned
            FROM threads
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
            })
        })?;

        let mut threads = Vec::new();
        for row in rows {
            threads.push(row?);
        }
        Ok(threads)
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
            INSERT INTO peers (id, alias, friendcode, iroh_peer_id, gpg_fingerprint, last_seen, trust_state, avatar_file_id, username, bio)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT(id) DO UPDATE SET
                alias = excluded.alias,
                friendcode = excluded.friendcode,
                iroh_peer_id = excluded.iroh_peer_id,
                gpg_fingerprint = excluded.gpg_fingerprint,
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
                SELECT id, alias, friendcode, iroh_peer_id, gpg_fingerprint, last_seen, trust_state, avatar_file_id, username, bio
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
                        last_seen: row.get(5)?,
                        trust_state: row.get(6)?,
                        avatar_file_id: row.get(7)?,
                        username: row.get(8)?,
                        bio: row.get(9)?,
                    })
                },
            )
            .optional()?;
        Ok(row)
    }

    fn list(&self) -> Result<Vec<PeerRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, alias, friendcode, iroh_peer_id, gpg_fingerprint, last_seen, trust_state, avatar_file_id, username, bio
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
                last_seen: row.get(5)?,
                trust_state: row.get(6)?,
                avatar_file_id: row.get(7)?,
                username: row.get(8)?,
                bio: row.get(9)?,
            })
        })?;
        let mut peers = Vec::new();
        for row in rows {
            peers.push(row?);
        }
        Ok(peers)
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
