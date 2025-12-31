use crate::database::models::{
    BlockedPeerRecord, BlocklistEntryRecord, BlocklistSubscriptionRecord, RedactedPostRecord,
    IpBlockRecord,
};
use crate::database::repositories::{
    BlockedPeerRepository, BlocklistRepository, PeerRepository, RedactedPostRepository,
    IpBlockRepository, PeerIpRepository,
};
use crate::database::Database;
use crate::utils::now_utc_iso;
use anyhow::{anyhow, Context, Result};
use ipnetwork::IpNetwork;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

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

/// Fast IP blocking checker with in-memory caching
///
/// Performance characteristics:
/// - Exact IP blocks: O(1) lookup via HashSet
/// - CIDR range blocks: O(n) where n = number of CIDR rules (typically small)
/// - Cache refresh: Only when blocks are added/removed
#[derive(Clone)]
pub struct IpBlockChecker {
    database: Database,
    cache: Arc<RwLock<IpBlockCache>>,
}

struct IpBlockCache {
    /// Fast O(1) lookup for exact IP matches
    exact_blocks: HashSet<IpAddr>,

    /// CIDR range blocks (small list, fast to check)
    range_blocks: Vec<CidrBlock>,

    /// Map block ID to metadata for hit count updates
    block_metadata: HashMap<i64, IpBlockRecord>,
}

struct CidrBlock {
    id: i64,
    network: IpNetwork,
}

impl IpBlockChecker {
    pub fn new(database: Database) -> Self {
        Self {
            database,
            cache: Arc::new(RwLock::new(IpBlockCache {
                exact_blocks: HashSet::new(),
                range_blocks: Vec::new(),
                block_metadata: HashMap::new(),
            })),
        }
    }

    /// Initialize the cache by loading all active blocks from database
    pub async fn load_cache(&self) -> Result<()> {
        let blocks = self.database.with_repositories(|repos| {
            repos.ip_blocks().list_active()
        })?;

        let mut cache = self.cache.write().await;
        cache.exact_blocks.clear();
        cache.range_blocks.clear();
        cache.block_metadata.clear();

        for block in blocks {
            cache.block_metadata.insert(block.id, block.clone());

            match block.block_type.as_str() {
                "exact" => {
                    if let Ok(ip) = block.ip_or_range.parse::<IpAddr>() {
                        cache.exact_blocks.insert(ip);
                    } else {
                        tracing::warn!(
                            block_id = block.id,
                            ip = %block.ip_or_range,
                            "invalid IP address in exact block"
                        );
                    }
                }
                "range" => {
                    match block.ip_or_range.parse::<IpNetwork>() {
                        Ok(network) => {
                            cache.range_blocks.push(CidrBlock {
                                id: block.id,
                                network,
                            });
                        }
                        Err(err) => {
                            tracing::warn!(
                                error = ?err,
                                block_id = block.id,
                                range = %block.ip_or_range,
                                "invalid CIDR range in range block"
                            );
                        }
                    }
                }
                _ => {
                    tracing::warn!(
                        block_id = block.id,
                        block_type = %block.block_type,
                        "unknown block type"
                    );
                }
            }
        }

        tracing::info!(
            exact_count = cache.exact_blocks.len(),
            range_count = cache.range_blocks.len(),
            "loaded IP blocks into cache"
        );

        Ok(())
    }

    /// Check if an IP address is blocked
    ///
    /// Returns (is_blocked, block_id) where block_id is Some if blocked
    pub async fn is_blocked(&self, ip: &IpAddr) -> Result<(bool, Option<i64>)> {
        let cache = self.cache.read().await;

        // Fast path: check exact IP match (O(1))
        if cache.exact_blocks.contains(ip) {
            // Find the block ID for hit count tracking
            if let Some((id, _)) = cache.block_metadata.iter()
                .find(|(_, block)| block.block_type == "exact" && block.ip_or_range == ip.to_string())
            {
                return Ok((true, Some(*id)));
            }
            return Ok((true, None));
        }

        // Slower path: check CIDR ranges (O(n) but n is small)
        for cidr_block in &cache.range_blocks {
            if cidr_block.network.contains(*ip) {
                return Ok((true, Some(cidr_block.id)));
            }
        }

        Ok((false, None))
    }

    /// Check if a peer is blocked based on their known IP addresses
    ///
    /// Returns (is_blocked, block_id, ip) where ip is the blocked IP if any
    pub async fn is_peer_blocked(&self, peer_id: &str) -> Result<(bool, Option<i64>, Option<IpAddr>)> {
        // Look up peer's known IP addresses
        let peer_ip_record = self.database.with_repositories(|repos| {
            repos.peer_ips().get(peer_id)
        })?;

        if let Some(record) = peer_ip_record {
            if let Ok(ip) = record.ip_address.parse::<IpAddr>() {
                let (is_blocked, block_id) = self.is_blocked(&ip).await?;
                if is_blocked {
                    return Ok((true, block_id, Some(ip)));
                }
            }
        }

        Ok((false, None, None))
    }

    /// Add a new IP block (exact IP or CIDR range)
    pub async fn add_block(&self, ip_or_range: &str, reason: Option<String>) -> Result<i64> {
        // Determine block type
        let block_type = if ip_or_range.contains('/') {
            // Validate CIDR
            ip_or_range.parse::<IpNetwork>()
                .context("Invalid CIDR range")?;
            "range"
        } else {
            // Validate IP
            ip_or_range.parse::<IpAddr>()
                .context("Invalid IP address")?;
            "exact"
        };

        let record = IpBlockRecord {
            id: 0, // Will be assigned by database
            ip_or_range: ip_or_range.to_string(),
            block_type: block_type.to_string(),
            blocked_at: chrono::Utc::now().timestamp(),
            reason,
            active: true,
            hit_count: 0,
        };

        let block_id = self.database.with_repositories(|repos| {
            repos.ip_blocks().add(&record)
        })?;

        // Refresh cache
        self.load_cache().await?;

        tracing::info!(
            block_id = block_id,
            ip_or_range = %ip_or_range,
            block_type = %block_type,
            "added IP block"
        );

        Ok(block_id)
    }

    /// Remove an IP block
    pub async fn remove_block(&self, block_id: i64) -> Result<()> {
        self.database.with_repositories(|repos| {
            repos.ip_blocks().remove(block_id)
        })?;

        // Refresh cache
        self.load_cache().await?;

        tracing::info!(block_id = block_id, "removed IP block");

        Ok(())
    }

    /// Increment hit count for a block (called when a block triggers)
    pub async fn record_hit(&self, block_id: i64) -> Result<()> {
        self.database.with_repositories(|repos| {
            repos.ip_blocks().increment_hit_count(block_id)
        })?;

        Ok(())
    }

    /// List all IP blocks (active and inactive)
    pub fn list_all(&self) -> Result<Vec<IpBlockRecord>> {
        self.database.with_repositories(|repos| {
            repos.ip_blocks().list_all()
        })
    }

    /// List only active IP blocks
    pub fn list_active(&self) -> Result<Vec<IpBlockRecord>> {
        self.database.with_repositories(|repos| {
            repos.ip_blocks().list_active()
        })
    }
}
