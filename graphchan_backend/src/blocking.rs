use crate::database::models::{
    BlockedPeerRecord, BlocklistEntryRecord, BlocklistSubscriptionRecord, RedactedPostRecord,
};
use crate::database::repositories::{
    BlockedPeerRepository, BlocklistRepository, PeerRepository, RedactedPostRepository,
};
use crate::database::Database;
use crate::utils::now_utc_iso;
use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct BlockChecker {
    database: Database,
}

impl BlockChecker {
    pub fn new(database: Database) -> Self {
        Self { database }
    }

    /// Check if a peer is blocked (either directly or via any subscribed blocklist).
    pub fn is_blocked(&self, peer_id: &str) -> Result<bool> {
        self.database.with_repositories(|repos| {
            // Check direct blocks first
            if repos.blocked_peers().is_blocked(peer_id)? {
                return Ok(true);
            }

            // Check if peer is in any subscribed blocklist with auto-apply enabled
            repos.blocklists().is_in_any_blocklist(peer_id)
        })
    }

    /// Block a peer directly with an optional reason.
    pub fn block_peer(&self, peer_id: &str, reason: Option<String>) -> Result<()> {
        // Verify peer exists
        self.database.with_repositories(|repos| {
            repos
                .peers()
                .get(peer_id)?
                .ok_or_else(|| anyhow!("peer not found: {}", peer_id))?;

            let record = BlockedPeerRecord {
                peer_id: peer_id.to_string(),
                reason,
                blocked_at: now_utc_iso(),
            };

            repos.blocked_peers().block(&record)
        })
    }

    /// Unblock a peer.
    pub fn unblock_peer(&self, peer_id: &str) -> Result<()> {
        self.database
            .with_repositories(|repos| repos.blocked_peers().unblock(peer_id))
    }

    /// List all directly blocked peers.
    pub fn list_blocked_peers(&self) -> Result<Vec<BlockedPeerView>> {
        self.database.with_repositories(|repos| {
            let records = repos.blocked_peers().list()?;
            let mut views = Vec::new();

            for record in records {
                // Get peer info if available
                let peer = repos.peers().get(&record.peer_id)?;
                views.push(BlockedPeerView {
                    peer_id: record.peer_id,
                    peer_username: peer.as_ref().and_then(|p| p.username.clone()),
                    peer_alias: peer.as_ref().and_then(|p| p.alias.clone()),
                    reason: record.reason,
                    blocked_at: record.blocked_at,
                });
            }

            Ok(views)
        })
    }

    /// Subscribe to a blocklist.
    pub fn subscribe_blocklist(
        &self,
        blocklist_id: &str,
        maintainer_peer_id: &str,
        name: String,
        description: Option<String>,
        auto_apply: bool,
    ) -> Result<()> {
        self.database.with_repositories(|repos| {
            // Verify maintainer peer exists
            repos
                .peers()
                .get(maintainer_peer_id)?
                .ok_or_else(|| anyhow!("maintainer peer not found: {}", maintainer_peer_id))?;

            let record = BlocklistSubscriptionRecord {
                id: blocklist_id.to_string(),
                maintainer_peer_id: maintainer_peer_id.to_string(),
                name,
                description,
                auto_apply,
                last_synced_at: None,
            };

            repos.blocklists().subscribe(&record)
        })
    }

    /// Unsubscribe from a blocklist.
    pub fn unsubscribe_blocklist(&self, blocklist_id: &str) -> Result<()> {
        self.database
            .with_repositories(|repos| repos.blocklists().unsubscribe(blocklist_id))
    }

    /// List all blocklist subscriptions.
    pub fn list_blocklist_subscriptions(&self) -> Result<Vec<BlocklistSubscriptionView>> {
        self.database.with_repositories(|repos| {
            let records = repos.blocklists().list_subscriptions()?;
            let mut views = Vec::new();

            for record in records {
                // Count entries in this blocklist
                let entries = repos.blocklists().list_entries(&record.id)?;
                let entry_count = entries.len();

                views.push(BlocklistSubscriptionView {
                    id: record.id,
                    maintainer_peer_id: record.maintainer_peer_id,
                    name: record.name,
                    description: record.description,
                    auto_apply: record.auto_apply,
                    last_synced_at: record.last_synced_at,
                    entry_count,
                });
            }

            Ok(views)
        })
    }

    /// Add an entry to a blocklist (typically called when syncing from maintainer).
    pub fn add_blocklist_entry(
        &self,
        blocklist_id: &str,
        peer_id: &str,
        reason: Option<String>,
    ) -> Result<()> {
        self.database.with_repositories(|repos| {
            let entry = BlocklistEntryRecord {
                blocklist_id: blocklist_id.to_string(),
                peer_id: peer_id.to_string(),
                reason,
                added_at: now_utc_iso(),
            };

            repos.blocklists().add_entry(&entry)
        })
    }

    /// Remove an entry from a blocklist.
    pub fn remove_blocklist_entry(&self, blocklist_id: &str, peer_id: &str) -> Result<()> {
        self.database
            .with_repositories(|repos| repos.blocklists().remove_entry(blocklist_id, peer_id))
    }

    /// Get entries for a specific blocklist.
    pub fn list_blocklist_entries(&self, blocklist_id: &str) -> Result<Vec<BlocklistEntryView>> {
        self.database.with_repositories(|repos| {
            let entries = repos.blocklists().list_entries(blocklist_id)?;
            let mut views = Vec::new();

            for entry in entries {
                // Get peer info if available
                let peer = repos.peers().get(&entry.peer_id)?;
                views.push(BlocklistEntryView {
                    peer_id: entry.peer_id,
                    peer_username: peer.as_ref().and_then(|p| p.username.clone()),
                    peer_alias: peer.as_ref().and_then(|p| p.alias.clone()),
                    reason: entry.reason,
                    added_at: entry.added_at,
                });
            }

            Ok(views)
        })
    }

    /// Create a redacted post placeholder to preserve DAG structure.
    pub fn create_redacted_post(
        &self,
        post_id: &str,
        thread_id: &str,
        author_peer_id: &str,
        parent_post_ids: Vec<String>,
        known_child_ids: Option<Vec<String>>,
        redaction_reason: &str,
    ) -> Result<()> {
        self.database.with_repositories(|repos| {
            let record = RedactedPostRecord {
                id: post_id.to_string(),
                thread_id: thread_id.to_string(),
                author_peer_id: author_peer_id.to_string(),
                parent_post_ids: serde_json::to_string(&parent_post_ids)?,
                known_child_ids: known_child_ids
                    .map(|ids| serde_json::to_string(&ids))
                    .transpose()?,
                redaction_reason: redaction_reason.to_string(),
                discovered_at: now_utc_iso(),
            };

            repos.redacted_posts().create(&record)
        })
    }

    /// Get a redacted post by ID.
    pub fn get_redacted_post(&self, post_id: &str) -> Result<Option<RedactedPostView>> {
        self.database.with_repositories(|repos| {
            match repos.redacted_posts().get(post_id)? {
                Some(record) => {
                    let parent_post_ids: Vec<String> =
                        serde_json::from_str(&record.parent_post_ids)?;
                    let known_child_ids: Option<Vec<String>> = record
                        .known_child_ids
                        .map(|ids| serde_json::from_str(&ids))
                        .transpose()?;

                    Ok(Some(RedactedPostView {
                        id: record.id,
                        thread_id: record.thread_id,
                        author_peer_id: record.author_peer_id,
                        parent_post_ids,
                        known_child_ids,
                        redaction_reason: record.redaction_reason,
                        discovered_at: record.discovered_at,
                    }))
                }
                None => Ok(None),
            }
        })
    }

    /// List all redacted posts for a thread.
    pub fn list_redacted_posts_for_thread(
        &self,
        thread_id: &str,
    ) -> Result<Vec<RedactedPostView>> {
        self.database.with_repositories(|repos| {
            let records = repos.redacted_posts().list_for_thread(thread_id)?;
            let mut views = Vec::new();

            for record in records {
                let parent_post_ids: Vec<String> = serde_json::from_str(&record.parent_post_ids)?;
                let known_child_ids: Option<Vec<String>> = record
                    .known_child_ids
                    .map(|ids| serde_json::from_str(&ids))
                    .transpose()?;

                views.push(RedactedPostView {
                    id: record.id,
                    thread_id: record.thread_id,
                    author_peer_id: record.author_peer_id,
                    parent_post_ids,
                    known_child_ids,
                    redaction_reason: record.redaction_reason,
                    discovered_at: record.discovered_at,
                });
            }

            Ok(views)
        })
    }

    /// Check if content should be filtered based on blocking rules.
    /// Returns Ok(()) if content is allowed, Err with reason if blocked.
    pub fn check_content_allowed(&self, peer_id: &str) -> Result<()> {
        if self.is_blocked(peer_id)? {
            anyhow::bail!("content from blocked peer: {}", peer_id);
        }
        Ok(())
    }
}

/// View model for a blocked peer with enriched peer info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlockedPeerView {
    pub peer_id: String,
    pub peer_username: Option<String>,
    pub peer_alias: Option<String>,
    pub reason: Option<String>,
    pub blocked_at: String,
}

/// View model for a blocklist subscription with entry count.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistSubscriptionView {
    pub id: String,
    pub maintainer_peer_id: String,
    pub name: String,
    pub description: Option<String>,
    pub auto_apply: bool,
    pub last_synced_at: Option<String>,
    pub entry_count: usize,
}

/// View model for a blocklist entry with peer info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlocklistEntryView {
    pub peer_id: String,
    pub peer_username: Option<String>,
    pub peer_alias: Option<String>,
    pub reason: Option<String>,
    pub added_at: String,
}

/// View model for a redacted post.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactedPostView {
    pub id: String,
    pub thread_id: String,
    pub author_peer_id: String,
    pub parent_post_ids: Vec<String>,
    pub known_child_ids: Option<Vec<String>>,
    pub redaction_reason: String,
    pub discovered_at: String,
}
