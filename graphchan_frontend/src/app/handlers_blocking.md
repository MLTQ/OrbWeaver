# handlers_blocking.rs

## Purpose
Handles all async message responses related to peer blocking, blocklist management, and IP blocking. Called by the dispatcher in `messages.rs` for blocking-related `AppMessage` variants.

## Components

### Peer Blocking Handlers
- `handle_blocked_peers_loaded` - Updates `blocking_state.blocked_peers` list
- `handle_peer_blocked` - Appends new block, clears form inputs
- `handle_peer_unblocked` - Removes peer from blocked list by ID

### Blocklist Handlers
- `handle_blocklists_loaded` - Updates `blocking_state.blocklists`
- `handle_blocklist_subscribed` - Appends subscription, clears form, resets `auto_apply`
- `handle_blocklist_unsubscribed` - Removes blocklist by ID
- `handle_blocklist_entries_loaded` - Updates `blocking_state.blocklist_entries`

### IP Blocking Handlers
- `handle_ip_blocks_loaded` - Updates `blocking_state.ip_blocks` list
- `handle_ip_block_stats_loaded` - Updates `blocking_state.ip_block_stats`
- `handle_ip_block_added` - Clears form, triggers reload of blocks + stats
- `handle_ip_block_removed` - Removes block by ID, refreshes stats
- `handle_ip_blocks_imported` - Clears import text, refreshes blocks + stats
- `handle_ip_blocks_exported` - Logs export (clipboard/save TODO)
- `handle_ip_blocks_cleared` - Clears local list, refreshes stats
- `handle_peer_ip_blocked` - Shows success banner, refreshes blocks + stats
- `handle_peer_ip_block_failed` - Shows error banner

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `messages.rs` | All 17 handler methods exist on `GraphchanApp` | Method removal/rename |
| `spawners.rs` | Handlers call `spawn_load_ip_blocks`, `spawn_load_ip_block_stats` | Spawner removal |
| `state.rs` | `BlockingState` fields match handler expectations | Field removal/rename |

## Notes
- IP block mutations (add/remove/import/clear) trigger reload of both blocks list and stats
- `handle_ip_blocks_exported` currently only logs; clipboard/file-save UI is TODO
- All errors logged via `log::error!` and stored in appropriate `*_error` fields on `BlockingState`
