use log::{error, info};

use crate::models::{BlockedPeerView, BlocklistEntryView, BlocklistSubscriptionView};

use super::GraphchanApp;

impl GraphchanApp {
    pub(super) fn handle_blocked_peers_loaded(&mut self, result: Result<Vec<BlockedPeerView>, anyhow::Error>) {
        self.blocking_state.blocked_peers_loading = false;
        match result {
            Ok(peers) => {
                self.blocking_state.blocked_peers = peers;
                self.blocking_state.blocked_peers_error = None;
            }
            Err(err) => {
                error!("Failed to load blocked peers: {}", err);
                self.blocking_state.blocked_peers_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_peer_blocked(&mut self, peer_id: String, result: Result<BlockedPeerView, anyhow::Error>) {
        self.blocking_state.blocking_in_progress = false;
        match result {
            Ok(blocked) => {
                self.blocking_state.blocked_peers.push(blocked);
                self.blocking_state.new_block_peer_id.clear();
                self.blocking_state.new_block_reason.clear();
                self.blocking_state.block_error = None;
            }
            Err(err) => {
                error!("Failed to block peer {}: {}", peer_id, err);
                self.blocking_state.block_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_peer_unblocked(&mut self, peer_id: String, result: Result<(), anyhow::Error>) {
        match result {
            Ok(_) => {
                self.blocking_state.blocked_peers.retain(|p| p.peer_id != peer_id);
            }
            Err(err) => {
                error!("Failed to unblock peer {}: {}", peer_id, err);
                self.blocking_state.blocked_peers_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_blocklists_loaded(&mut self, result: Result<Vec<BlocklistSubscriptionView>, anyhow::Error>) {
        self.blocking_state.blocklists_loading = false;
        match result {
            Ok(blocklists) => {
                self.blocking_state.blocklists = blocklists;
                self.blocking_state.blocklists_error = None;
            }
            Err(err) => {
                error!("Failed to load blocklists: {}", err);
                self.blocking_state.blocklists_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_blocklist_subscribed(&mut self, blocklist_id: String, result: Result<BlocklistSubscriptionView, anyhow::Error>) {
        self.blocking_state.subscribing_in_progress = false;
        match result {
            Ok(subscription) => {
                self.blocking_state.blocklists.push(subscription);
                self.blocking_state.new_blocklist_id.clear();
                self.blocking_state.new_blocklist_maintainer.clear();
                self.blocking_state.new_blocklist_name.clear();
                self.blocking_state.new_blocklist_description.clear();
                self.blocking_state.new_blocklist_auto_apply = false;
                self.blocking_state.subscribe_error = None;
            }
            Err(err) => {
                error!("Failed to subscribe to blocklist {}: {}", blocklist_id, err);
                self.blocking_state.subscribe_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_blocklist_unsubscribed(&mut self, blocklist_id: String, result: Result<(), anyhow::Error>) {
        match result {
            Ok(_) => {
                self.blocking_state.blocklists.retain(|b| b.id != blocklist_id);
            }
            Err(err) => {
                error!("Failed to unsubscribe from blocklist {}: {}", blocklist_id, err);
                self.blocking_state.blocklists_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_blocklist_entries_loaded(&mut self, blocklist_id: String, result: Result<Vec<BlocklistEntryView>, anyhow::Error>) {
        self.blocking_state.blocklist_entries_loading = false;
        match result {
            Ok(entries) => {
                self.blocking_state.blocklist_entries = entries;
                self.blocking_state.blocklist_entries_error = None;
            }
            Err(err) => {
                error!("Failed to load entries for blocklist {}: {}", blocklist_id, err);
                self.blocking_state.blocklist_entries_error = Some(err.to_string());
            }
        }
    }

    // IP Blocking handlers

    pub(super) fn handle_ip_blocks_loaded(&mut self, result: Result<Vec<crate::models::IpBlockView>, anyhow::Error>) {
        self.blocking_state.ip_blocks_loading = false;
        match result {
            Ok(blocks) => {
                self.blocking_state.ip_blocks = blocks;
                self.blocking_state.ip_blocks_error = None;
            }
            Err(err) => {
                error!("Failed to load IP blocks: {}", err);
                self.blocking_state.ip_blocks_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_ip_block_stats_loaded(&mut self, result: Result<crate::models::IpBlockStatsResponse, anyhow::Error>) {
        self.blocking_state.ip_block_stats_loading = false;
        match result {
            Ok(stats) => {
                self.blocking_state.ip_block_stats = Some(stats);
            }
            Err(err) => {
                error!("Failed to load IP block stats: {}", err);
            }
        }
    }

    pub(super) fn handle_ip_block_added(&mut self, result: Result<(), anyhow::Error>) {
        self.blocking_state.adding_ip_block = false;
        match result {
            Ok(_) => {
                self.blocking_state.new_ip_block.clear();
                self.blocking_state.new_ip_block_reason.clear();
                self.blocking_state.add_ip_block_error = None;
                self.spawn_load_ip_blocks();
                self.spawn_load_ip_block_stats();
            }
            Err(err) => {
                error!("Failed to add IP block: {}", err);
                self.blocking_state.add_ip_block_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_ip_block_removed(&mut self, block_id: i64, result: Result<(), anyhow::Error>) {
        match result {
            Ok(_) => {
                self.blocking_state.ip_blocks.retain(|b| b.id != block_id);
                self.spawn_load_ip_block_stats();
            }
            Err(err) => {
                error!("Failed to remove IP block {}: {}", block_id, err);
                self.blocking_state.ip_blocks_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_ip_blocks_imported(&mut self, result: Result<(), anyhow::Error>) {
        self.blocking_state.importing_ips = false;
        match result {
            Ok(_) => {
                self.blocking_state.import_text.clear();
                self.blocking_state.import_error = None;
                self.spawn_load_ip_blocks();
                self.spawn_load_ip_block_stats();
            }
            Err(err) => {
                error!("Failed to import IP blocks: {}", err);
                self.blocking_state.import_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_ip_blocks_exported(&mut self, result: Result<String, anyhow::Error>) {
        match result {
            Ok(export_text) => {
                // Copy to clipboard or save to file
                // For now, just log it
                info!("IP blocks exported: {} bytes", export_text.len());
                // TODO: Add clipboard or save dialog
            }
            Err(err) => {
                error!("Failed to export IP blocks: {}", err);
            }
        }
    }

    pub(super) fn handle_ip_blocks_cleared(&mut self, result: Result<(), anyhow::Error>) {
        match result {
            Ok(_) => {
                self.blocking_state.ip_blocks.clear();
                self.spawn_load_ip_block_stats();
            }
            Err(err) => {
                error!("Failed to clear IP blocks: {}", err);
                self.blocking_state.ip_blocks_error = Some(err.to_string());
            }
        }
    }

    pub(super) fn handle_peer_ip_blocked(&mut self, peer_id: String, blocked_ips: Vec<String>) {
        info!("Blocked {} IP(s) for peer {}: {:?}", blocked_ips.len(), peer_id, blocked_ips);
        // Refresh IP blocks list
        self.spawn_load_ip_blocks();
        self.spawn_load_ip_block_stats();
        // Show success message
        self.info_banner = Some(format!("Blocked {} IP(s) for peer {}", blocked_ips.len(), peer_id));
    }

    pub(super) fn handle_peer_ip_block_failed(&mut self, peer_id: String, error: String) {
        error!("Failed to block IPs for peer {}: {}", peer_id, error);
        self.info_banner = Some(format!("Failed to block IPs for peer {}: {}", peer_id, error));
    }
}
