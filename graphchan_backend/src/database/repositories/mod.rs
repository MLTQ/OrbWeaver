mod files;
mod import_post_map;
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
    BlockedPeerRecord, BlocklistEntryRecord, BlocklistSubscriptionRecord, ConversationRecord,
    DirectMessageRecord, FileRecord, IpBlockRecord, PeerIpRecord, PeerRecord, PostRecord,
    ReactionRecord, RedactedPostRecord, SearchResultRecord, ThreadMemberKey, ThreadRecord,
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
    fn set_source_info(&self, thread_id: &str, source_url: &str, platform: &str) -> Result<()>;
    fn set_last_refreshed(&self, thread_id: &str) -> Result<()>;
}

pub trait ImportPostMapRepository {
    fn insert(&self, thread_id: &str, external_id: &str, internal_id: &str) -> Result<()>;
    fn get_map(&self, thread_id: &str) -> Result<HashMap<String, String>>;
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
    /// Resolve the canonical peer id (GPG fingerprint) from an iroh public key
    /// string, used when an inbound gossip frame only carries the iroh-side
    /// identifier and we need to record IP/connection info against a peer row.
    fn id_for_iroh_peer(&self, iroh_peer_id: &str) -> Result<Option<String>>;
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
    fn list_for_conversation(
        &self,
        conversation_id: &str,
        limit: usize,
    ) -> Result<Vec<DirectMessageRecord>>;
    fn mark_as_read(&self, id: &str, read_at: &str) -> Result<()>;
    /// Mark every unread incoming message in a conversation as read.
    /// Returns the number of rows updated. Pairs with
    /// ConversationRepository::update_unread_count(0).
    fn mark_conversation_read(
        &self,
        conversation_id: &str,
        to_peer_id: &str,
        read_at: &str,
    ) -> Result<usize>;
    fn count_unread(&self, to_peer_id: &str) -> Result<usize>;
    /// Update the decrypt_status of a single message. Used by the retry path
    /// when a peer's x25519 key becomes available after their DM had already
    /// landed in 'pending_key' state.
    fn update_decrypt_status(&self, id: &str, status: &str) -> Result<()>;
    /// Return DMs from this peer that we couldn't decrypt before — the retry
    /// candidates. Excludes 'failed' messages (non-key failures we won't
    /// recover from).
    fn list_pending_for_sender(&self, from_peer_id: &str) -> Result<Vec<DirectMessageRecord>>;
}

pub trait ConversationRepository {
    fn upsert(&self, record: &ConversationRecord) -> Result<()>;
    fn get(&self, id: &str) -> Result<Option<ConversationRecord>>;
    fn list(&self) -> Result<Vec<ConversationRecord>>;
    fn update_unread_count(&self, conversation_id: &str, count: i64) -> Result<()>;
    fn update_last_message(
        &self,
        conversation_id: &str,
        message_at: &str,
        preview: &str,
    ) -> Result<()>;
    /// Atomically increment unread_count by 1, creating the conversation row if
    /// missing and updating last_message_at / preview in the same statement.
    /// Used by receive_dm — separate from upsert() because upsert clobbers the
    /// counter, which is a footgun when state is concurrent.
    fn record_incoming_message(
        &self,
        conversation_id: &str,
        peer_id: &str,
        message_at: &str,
        preview: &str,
    ) -> Result<()>;
    /// Update last_message_at + preview without touching unread_count. Used by
    /// send_dm so replying does not silently clear unread state from the other
    /// side of the conversation.
    fn record_outgoing_message(
        &self,
        conversation_id: &str,
        peer_id: &str,
        message_at: &str,
        preview: &str,
    ) -> Result<()>;
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

    pub fn import_post_map(&self) -> impl ImportPostMapRepository + '_ {
        import_post_map::SqliteImportPostMapRepository { conn: self.conn }
    }

    pub fn conn(&self) -> &'conn Connection {
        self.conn
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_db() -> crate::database::Database {
        // Drive the real migration runner so fixtures stay current as schema evolves.
        let db = crate::database::Database::from_connection(
            Connection::open_in_memory().expect("in-memory db"),
            true,
        );
        db.ensure_migrations().expect("migrations");
        db
    }

    fn make_peer(id: &str, alias: Option<&str>) -> PeerRecord {
        PeerRecord {
            id: id.into(),
            alias: alias.map(Into::into),
            username: None,
            bio: None,
            friendcode: None,
            iroh_peer_id: None,
            gpg_fingerprint: None,
            x25519_pubkey: None,
            last_seen: None,
            trust_state: "unknown".into(),
            avatar_file_id: None,
            agents: None,
        }
    }

    fn make_thread(id: &str, title: &str, creator: &str) -> ThreadRecord {
        ThreadRecord {
            id: id.into(),
            title: title.into(),
            creator_peer_id: Some(creator.into()),
            created_at: "2024-01-01T00:00:00Z".into(),
            pinned: false,
            thread_hash: None,
            visibility: "social".into(),
            topic_secret: None,
            sync_status: "downloaded".into(),
            source_url: None,
            source_platform: None,
            last_refreshed_at: None,
        }
    }

    fn make_post(id: &str, thread_id: &str, author: &str, body: &str) -> PostRecord {
        PostRecord {
            id: id.into(),
            thread_id: thread_id.into(),
            author_peer_id: Some(author.into()),
            author_friendcode: None,
            body: body.into(),
            created_at: "2024-01-01T00:00:01Z".into(),
            updated_at: None,
            metadata: None,
        }
    }

    #[test]
    fn thread_and_post_repositories_work() {
        let db = setup_db();
        db.with_repositories(|repos| {
            let peer = make_peer("peer-1", Some("author"));
            repos.peers().upsert(&peer)?;

            let thread = make_thread("thread-1", "First", &peer.id);
            repos.threads().create(&thread)?;

            let fetched = repos.threads().get("thread-1")?.unwrap();
            assert_eq!(fetched.title, "First");

            let post = make_post("post-1", &thread.id, &peer.id, "Hello");
            repos.posts().create(&post)?;

            let posts = repos.posts().list_for_thread(&thread.id)?;
            assert_eq!(posts.len(), 1);
            assert_eq!(posts[0].body, "Hello");
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn peer_and_file_repositories_work() {
        let db = setup_db();
        db.with_repositories(|repos| {
            let mut peer = make_peer("peer-1", Some("alice"));
            peer.friendcode = Some("friend-code".into());
            peer.iroh_peer_id = Some("peer-id".into());
            peer.gpg_fingerprint = Some("fingerprint".into());
            peer.last_seen = Some("2024-01-01T00:00:00Z".into());
            peer.trust_state = "trusted".into();
            repos.peers().upsert(&peer)?;
            let fetched = repos.peers().get("peer-1")?.unwrap();
            assert_eq!(fetched.alias.as_deref(), Some("alice"));

            let thread = make_thread("thread-1", "Downloads", &peer.id);
            repos.threads().create(&thread)?;

            let post = make_post("post-1", &thread.id, &peer.id, "Attachment");
            repos.posts().create(&post)?;

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
                download_status: Some("available".into()),
            };
            repos.files().attach(&file)?;
            let files = repos.files().list_for_post(&post.id)?;
            assert_eq!(files.len(), 1);
            assert_eq!(files[0].checksum.as_deref(), Some("sha256:deadbeef"));
            Ok(())
        })
        .unwrap();
    }
}
