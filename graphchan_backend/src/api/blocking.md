# blocking.rs (API handlers)

## Purpose
Axum HTTP handlers for peer blocking, blocklist subscriptions, and IP blocking. Broadcasts block actions over gossip for shared blocklist enforcement.

## Components

### Peer Blocking

#### `block_peer_handler`
- **Does**: Blocks a peer and broadcasts block action over gossip
- **Route**: `POST /blocking/peers/{peer_id}`
- **Interacts with**: `BlockChecker::block_peer`, `NetworkHandle::publish_block_action`

#### `unblock_peer_handler`
- **Does**: Unblocks a peer
- **Route**: `DELETE /blocking/peers/{peer_id}`

#### `list_blocked_peers_handler`
- **Does**: Returns all blocked peers
- **Route**: `GET /blocking/peers`

#### `export_peer_blocks_handler`
- **Does**: Exports blocked peers as CSV (peer_id,reason,blocked_at)
- **Route**: `GET /blocking/peers/export`

#### `import_peer_blocks_handler`
- **Does**: Imports blocked peers from CSV (peer_id,reason per line, header optional)
- **Route**: `POST /blocking/peers/import`

### Blocklist Subscriptions

#### `subscribe_blocklist_handler`
- **Does**: Subscribes to a peer's blocklist and joins their gossip topic
- **Route**: `POST /blocking/blocklists`
- **Interacts with**: `BlockChecker::subscribe_blocklist`, `NetworkHandle::subscribe_to_peer`

#### `unsubscribe_blocklist_handler`
- **Does**: Removes blocklist subscription
- **Route**: `DELETE /blocking/blocklists/{id}`

#### `list_blocklists_handler` / `list_blocklist_entries_handler`
- **Does**: Lists subscriptions and their entries

### IP Blocking

#### `add_ip_block_handler` / `remove_ip_block_handler`
- **Does**: Add/remove IP or CIDR range blocks
- **Validates**: IP address or CIDR format
- **Reloads**: `IpBlockChecker` cache after changes

#### `import_ip_blocks_handler` / `export_ip_blocks_handler`
- **Does**: Bulk import/export IP blocks in text format (`IP # reason`)

#### `clear_all_ip_blocks_handler` / `ip_block_stats_handler`
- **Does**: Clear all blocks / show statistics

#### `get_peer_ip_handler`
- **Does**: Returns known IPs for a peer

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api/mod.rs` | Handler function signatures match Axum router | Signature changes |
| `BlockChecker` | `block_peer`, `list_blocklist_subscriptions`, etc. | Method signatures |
| `NetworkHandle` | `publish_block_action`, `subscribe_to_peer` | Method signatures |
| `IpBlockChecker` | `load_cache` for refreshing after changes | Method signature |
