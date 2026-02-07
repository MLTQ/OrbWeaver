mod files;
mod ip_blocks;
mod peer_ips;
mod peers;
mod posts;
mod reactions;
mod thread_member_keys;
mod threads;
mod topics;

mod blocked_peers;
mod blocklists;
mod conversations;
mod direct_messages;
mod redacted_posts;
mod search;

use super::models::{
    FileRecord, PeerRecord, PostRecord, ReactionRecord, ThreadRecord, ThreadMemberKey,
    DirectMessageRecord, ConversationRecord, BlockedPeerRecord, BlocklistSubscriptionRecord,
    BlocklistEntryRecord, RedactedPostRecord, SearchResultRecord,
    PeerIpRecord, IpBlockRecord,
};
use anyhow::Result;
use rusqlite::Connection;
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
    fn list_recent(&self, limit: usize) -> Result<Vec<PostRecord>>;
    fn add_relationships(&self, child_id: &str, parent_ids: &[String]) -> Result<()>;
    fn parents_of(&self, child_id: &str) -> Result<Vec<String>>;
    fn has_children(&self, post_id: &str) -> Result<bool>;
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

pub trait SearchRepository {
    fn search(&self, query: &str, limit: usize) -> Result<Vec<SearchResultRecord>>;
}

pub trait PeerIpRepository {
    fn update(&self, peer_id: &str, ip_address: &str, last_seen: i64) -> Result<()>;
    fn get(&self, peer_id: &str) -> Result<Option<PeerIpRecord>>;
    fn get_by_ip(&self, ip_address: &str) -> Result<Vec<PeerIpRecord>>;
    fn get_ips(&self, peer_id: &str) -> Result<Vec<String>>;
    fn list_all(&self) -> Result<Vec<PeerIpRecord>>;
}

pub trait IpBlockRepository {
    fn add(&self, record: &IpBlockRecord) -> Result<i64>;
    fn remove(&self, id: i64) -> Result<()>;
    fn set_active(&self, id: i64, active: bool) -> Result<()>;
    fn increment_hit_count(&self, id: i64) -> Result<()>;
    fn list_active(&self) -> Result<Vec<IpBlockRecord>>;
    fn list_all(&self) -> Result<Vec<IpBlockRecord>>;
    fn get(&self, id: i64) -> Result<Option<IpBlockRecord>>;
}

pub trait TopicRepository {
    fn subscribe(&self, topic_id: &str) -> Result<()>;
    fn unsubscribe(&self, topic_id: &str) -> Result<()>;
    fn list_subscribed(&self) -> Result<Vec<String>>;
    fn is_subscribed(&self, topic_id: &str) -> Result<bool>;
    fn add_thread_topic(&self, thread_id: &str, topic_id: &str) -> Result<()>;
    fn remove_thread_topic(&self, thread_id: &str, topic_id: &str) -> Result<()>;
    fn list_thread_topics(&self, thread_id: &str) -> Result<Vec<String>>;
    fn list_threads_for_topic(&self, topic_id: &str) -> Result<Vec<String>>;
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
        threads::SqliteThreadRepository { conn: self.conn }
    }

    pub fn posts(&self) -> impl PostRepository + '_ {
        posts::SqlitePostRepository { conn: self.conn }
    }

    pub fn peers(&self) -> impl PeerRepository + '_ {
        peers::SqlitePeerRepository { conn: self.conn }
    }

    pub fn files(&self) -> impl FileRepository + '_ {
        files::SqliteFileRepository { conn: self.conn }
    }

    pub fn reactions(&self) -> impl ReactionRepository + '_ {
        reactions::SqliteReactionRepository { conn: self.conn }
    }

    pub fn thread_member_keys(&self) -> impl ThreadMemberKeyRepository + '_ {
        thread_member_keys::SqliteThreadMemberKeyRepository { conn: self.conn }
    }

    pub fn direct_messages(&self) -> impl DirectMessageRepository + '_ {
        direct_messages::SqliteDirectMessageRepository { conn: self.conn }
    }

    pub fn conversations(&self) -> impl ConversationRepository + '_ {
        conversations::SqliteConversationRepository { conn: self.conn }
    }

    pub fn blocked_peers(&self) -> impl BlockedPeerRepository + '_ {
        blocked_peers::SqliteBlockedPeerRepository { conn: self.conn }
    }

    pub fn blocklists(&self) -> impl BlocklistRepository + '_ {
        blocklists::SqliteBlocklistRepository { conn: self.conn }
    }

    pub fn redacted_posts(&self) -> impl RedactedPostRepository + '_ {
        redacted_posts::SqliteRedactedPostRepository { conn: self.conn }
    }

    pub fn search(&self) -> impl SearchRepository + '_ {
        search::SqliteSearchRepository { conn: self.conn }
    }

    pub fn peer_ips(&self) -> impl PeerIpRepository + '_ {
        peer_ips::SqlitePeerIpRepository { conn: self.conn }
    }

    pub fn ip_blocks(&self) -> impl IpBlockRepository + '_ {
        ip_blocks::SqliteIpBlockRepository { conn: self.conn }
    }

    pub fn topics(&self) -> impl TopicRepository + '_ {
        topics::SqliteTopicRepository { conn: self.conn }
    }

    pub fn conn(&self) -> &'conn Connection {
        self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::MIGRATIONS;

    fn setup_conn() -> Connection {
        let conn = Connection::open_in_memory().expect("in-memory db");
        conn.execute_batch(MIGRATIONS).expect("base migrations");

        // Apply additional migrations that are normally run by Database::ensure_migrations()
        // These will error if columns already exist, which is fine - we ignore the error
        let _ = conn.execute("ALTER TABLE peers ADD COLUMN x25519_pubkey TEXT", []);
        let _ = conn.execute("ALTER TABLE threads ADD COLUMN visibility TEXT DEFAULT 'social'", []);
        let _ = conn.execute("ALTER TABLE threads ADD COLUMN topic_secret TEXT", []);

        conn
    }

    #[test]
    fn thread_and_post_repositories_work() {
        let conn = setup_conn();
        let repos = SqliteRepositories::new(&conn);

        let peer = PeerRecord {
            id: "peer-1".into(),
            alias: Some("author".into()),
            username: None,
            bio: None,
            friendcode: None,
            iroh_peer_id: None,
            gpg_fingerprint: None,
            x25519_pubkey: None,
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
            visibility: "social".into(),
            topic_secret: None,
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
            username: None,
            bio: None,
            friendcode: Some("friend-code".into()),
            iroh_peer_id: Some("peer-id".into()),
            gpg_fingerprint: Some("fingerprint".into()),
            x25519_pubkey: None,
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
            visibility: "social".into(),
            topic_secret: None,
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
