use super::{AppState, ApiError};
use crate::blocking::{BlockChecker, BlockedPeerView, BlocklistEntryView, BlocklistSubscriptionView};
use crate::database::repositories::PeerIpRepository;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::blocking::IpBlockChecker;
use crate::database::models::IpBlockRecord;
use crate::database::repositories::IpBlockRepository;
use ipnetwork::IpNetwork;
use std::net::IpAddr;

type ApiResult<T> = Result<Json<T>, ApiError>;

#[derive(Debug, Deserialize)]
pub(crate) struct BlockPeerRequest {
    reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct SubscribeBlocklistRequest {
    maintainer_peer_id: String,
    name: String,
    description: Option<String>,
    #[serde(default = "default_auto_apply")]
    auto_apply: bool,
}

fn default_auto_apply() -> bool {
    true
}

#[derive(Debug, Deserialize)]
pub(crate) struct AddIpBlockRequest {
    ip_or_range: String,
    reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct IpBlockView {
    id: i64,
    ip_or_range: String,
    block_type: String,
    blocked_at: i64,
    reason: Option<String>,
    active: bool,
    hit_count: i64,
}

#[derive(Debug, Serialize)]
pub(crate) struct IpBlockStatsResponse {
    total_blocks: usize,
    active_blocks: usize,
    total_hits: i64,
    exact_ip_blocks: usize,
    range_blocks: usize,
}

#[derive(Debug, Serialize)]
pub(crate) struct PeerIpResponse {
    peer_id: String,
    ips: Vec<String>,
}

pub(crate) async fn list_blocked_peers_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<BlockedPeerView>> {
    let checker = BlockChecker::new(state.database.clone());
    let blocked = checker.list_blocked_peers().map_err(ApiError::Internal)?;
    Ok(Json(blocked))
}

pub(crate) async fn block_peer_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
    Json(payload): Json<BlockPeerRequest>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());
    checker
        .block_peer(&peer_id, payload.reason)
        .map_err(ApiError::Internal)?;

    // TODO: Broadcast block action to network (optional - for shared blocklists)

    Ok(StatusCode::OK)
}

pub(crate) async fn unblock_peer_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());
    checker.unblock_peer(&peer_id).map_err(ApiError::Internal)?;
    Ok(StatusCode::OK)
}

pub(crate) async fn list_blocklists_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<BlocklistSubscriptionView>> {
    let checker = BlockChecker::new(state.database.clone());
    let blocklists = checker
        .list_blocklist_subscriptions()
        .map_err(ApiError::Internal)?;
    Ok(Json(blocklists))
}

pub(crate) async fn subscribe_blocklist_handler(
    State(state): State<AppState>,
    Json(payload): Json<SubscribeBlocklistRequest>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());

    // Generate a blocklist ID from maintainer + name
    let blocklist_id = format!(
        "{}",
        blake3::hash(format!("blocklist:{}:{}", payload.maintainer_peer_id, payload.name).as_bytes())
    );

    checker
        .subscribe_blocklist(
            &blocklist_id,
            &payload.maintainer_peer_id,
            payload.name,
            payload.description,
            payload.auto_apply,
        )
        .map_err(ApiError::Internal)?;

    // TODO: Subscribe to blocklist updates from maintainer via gossip

    Ok(StatusCode::CREATED)
}

pub(crate) async fn unsubscribe_blocklist_handler(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let checker = BlockChecker::new(state.database.clone());
    checker
        .unsubscribe_blocklist(&id)
        .map_err(ApiError::Internal)?;
    Ok(StatusCode::OK)
}

pub(crate) async fn list_blocklist_entries_handler(
    State(state): State<AppState>,
    Path(blocklist_id): Path<String>,
) -> ApiResult<Vec<BlocklistEntryView>> {
    let checker = BlockChecker::new(state.database.clone());
    let entries = checker
        .list_blocklist_entries(&blocklist_id)
        .map_err(ApiError::Internal)?;
    Ok(Json(entries))
}

// IP Blocking handlers

pub(crate) async fn list_ip_blocks_handler(
    State(state): State<AppState>,
) -> ApiResult<Vec<IpBlockView>> {
    let blocks = state.database.with_repositories(|repos| {
        repos.ip_blocks().list_all()
    }).map_err(ApiError::Internal)?;

    let views = blocks.into_iter().map(|block| IpBlockView {
        id: block.id,
        ip_or_range: block.ip_or_range,
        block_type: block.block_type,
        blocked_at: block.blocked_at,
        reason: block.reason,
        active: block.active,
        hit_count: block.hit_count,
    }).collect();

    Ok(Json(views))
}

pub(crate) async fn add_ip_block_handler(
    State(state): State<AppState>,
    Json(payload): Json<AddIpBlockRequest>,
) -> Result<StatusCode, ApiError> {
    // Validate IP or CIDR range
    let (block_type, validated_ip_or_range) = if let Ok(ip) = payload.ip_or_range.parse::<IpAddr>() {
        ("exact".to_string(), ip.to_string())
    } else if let Ok(network) = payload.ip_or_range.parse::<IpNetwork>() {
        ("range".to_string(), network.to_string())
    } else {
        return Err(ApiError::BadRequest(
            format!("Invalid IP address or CIDR range: {}", payload.ip_or_range)
        ));
    };

    let record = IpBlockRecord {
        id: 0, // Will be assigned by database
        ip_or_range: validated_ip_or_range,
        block_type,
        blocked_at: chrono::Utc::now().timestamp(),
        reason: payload.reason,
        active: true,
        hit_count: 0,
    };

    state.database.with_repositories(|repos| {
        repos.ip_blocks().add(&record)
    }).map_err(ApiError::Internal)?;

    // Reload cache to include new block
    let ip_blocker = IpBlockChecker::new(state.database.clone());
    if let Err(err) = ip_blocker.load_cache().await {
        tracing::warn!(error = ?err, "failed to reload IP block cache");
    }

    Ok(StatusCode::CREATED)
}

pub(crate) async fn remove_ip_block_handler(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.database.with_repositories(|repos| {
        repos.ip_blocks().remove(id)
    }).map_err(ApiError::Internal)?;

    // Reload cache to remove block
    let ip_blocker = IpBlockChecker::new(state.database.clone());
    if let Err(err) = ip_blocker.load_cache().await {
        tracing::warn!(error = ?err, "failed to reload IP block cache");
    }

    Ok(StatusCode::OK)
}

pub(crate) async fn import_ip_blocks_handler(
    State(state): State<AppState>,
    body: String,
) -> Result<StatusCode, ApiError> {
    let mut added_count = 0;
    let mut error_count = 0;

    for line in body.lines() {
        let line = line.trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Parse line format: "IP_OR_RANGE [# reason]"
        let (ip_or_range, reason) = if let Some(hash_pos) = line.find('#') {
            let ip_part = line[..hash_pos].trim();
            let reason_part = line[hash_pos + 1..].trim();
            (ip_part, if reason_part.is_empty() { None } else { Some(reason_part.to_string()) })
        } else {
            (line, None)
        };

        // Validate and add block
        let (block_type, validated_ip_or_range) = if let Ok(ip) = ip_or_range.parse::<IpAddr>() {
            ("exact".to_string(), ip.to_string())
        } else if let Ok(network) = ip_or_range.parse::<IpNetwork>() {
            ("range".to_string(), network.to_string())
        } else {
            tracing::warn!(line = %line, "skipping invalid IP/range in import");
            error_count += 1;
            continue;
        };

        let record = IpBlockRecord {
            id: 0,
            ip_or_range: validated_ip_or_range,
            block_type,
            blocked_at: chrono::Utc::now().timestamp(),
            reason,
            active: true,
            hit_count: 0,
        };

        match state.database.with_repositories(|repos| {
            repos.ip_blocks().add(&record)
        }) {
            Ok(_) => added_count += 1,
            Err(err) => {
                tracing::warn!(error = ?err, ip_or_range = %record.ip_or_range, "failed to add IP block during import");
                error_count += 1;
            }
        }
    }

    tracing::info!(added = added_count, errors = error_count, "IP block import completed");

    // Reload cache
    let ip_blocker = IpBlockChecker::new(state.database.clone());
    if let Err(err) = ip_blocker.load_cache().await {
        tracing::warn!(error = ?err, "failed to reload IP block cache");
    }

    Ok(StatusCode::OK)
}

pub(crate) async fn export_ip_blocks_handler(
    State(state): State<AppState>,
) -> Result<String, ApiError> {
    let blocks = state.database.with_repositories(|repos| {
        repos.ip_blocks().list_all()
    }).map_err(ApiError::Internal)?;

    let mut output = String::new();
    output.push_str("# Graphchan IP Blocklist Export\n");
    output.push_str(&format!("# Exported: {}\n", chrono::Utc::now().to_rfc3339()));
    output.push_str(&format!("# Total blocks: {}\n\n", blocks.len()));

    for block in blocks {
        if let Some(reason) = &block.reason {
            output.push_str(&format!("{} # {}\n", block.ip_or_range, reason));
        } else {
            output.push_str(&format!("{}\n", block.ip_or_range));
        }
    }

    Ok(output)
}

pub(crate) async fn clear_all_ip_blocks_handler(
    State(state): State<AppState>,
) -> Result<StatusCode, ApiError> {
    // Get all blocks and remove them
    let blocks = state.database.with_repositories(|repos| {
        repos.ip_blocks().list_all()
    }).map_err(ApiError::Internal)?;

    for block in blocks {
        state.database.with_repositories(|repos| {
            repos.ip_blocks().remove(block.id)
        }).map_err(ApiError::Internal)?;
    }

    tracing::info!("All IP blocks cleared");

    // Reload cache
    let ip_blocker = IpBlockChecker::new(state.database.clone());
    if let Err(err) = ip_blocker.load_cache().await {
        tracing::warn!(error = ?err, "failed to reload IP block cache");
    }

    Ok(StatusCode::OK)
}

pub(crate) async fn ip_block_stats_handler(
    State(state): State<AppState>,
) -> ApiResult<IpBlockStatsResponse> {
    let blocks = state.database.with_repositories(|repos| {
        repos.ip_blocks().list_all()
    }).map_err(ApiError::Internal)?;

    let total_blocks = blocks.len();
    let active_blocks = blocks.iter().filter(|b| b.active).count();
    let total_hits: i64 = blocks.iter().map(|b| b.hit_count).sum();
    let exact_ip_blocks = blocks.iter().filter(|b| b.block_type == "exact").count();
    let range_blocks = blocks.iter().filter(|b| b.block_type == "range").count();

    Ok(Json(IpBlockStatsResponse {
        total_blocks,
        active_blocks,
        total_hits,
        exact_ip_blocks,
        range_blocks,
    }))
}

pub(crate) async fn get_peer_ip_handler(
    State(state): State<AppState>,
    Path(peer_id): Path<String>,
) -> ApiResult<PeerIpResponse> {
    let ips = state.database.with_repositories(|repos| {
        repos.peer_ips().get_ips(&peer_id)
    }).map_err(ApiError::Internal)?;

    Ok(Json(PeerIpResponse {
        peer_id,
        ips,
    }))
}
