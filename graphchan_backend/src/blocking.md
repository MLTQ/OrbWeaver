# blocking.rs

## Purpose
Content moderation through peer blocking, blocklist subscriptions, and IP-based restrictions. Provides `BlockChecker` for filtering content from blocked sources.

## Components

### `BlockChecker`
- **Does**: Checks if peers/IPs are blocked, manages block lists
- **Interacts with**: Multiple blocking-related repositories

### Block Checking

#### `is_blocked`
- **Does**: Checks if peer is blocked (direct or via blocklist)
- **Flow**: Check direct blocks → Check subscribed blocklists
- **Returns**: `bool`

#### `is_ip_blocked`
- **Does**: Checks if IP address is blocked
- **Interacts with**: IpBlockRepository
- **Supports**: CIDR ranges (e.g., "192.168.0.0/24")

### Direct Peer Blocking

#### `block_peer`
- **Does**: Directly blocks a peer with optional reason
- **Validates**: Peer must exist in database

#### `unblock_peer`
- **Does**: Removes direct block on peer

#### `list_blocked_peers`
- **Does**: Lists all directly blocked peers with metadata
- **Returns**: `Vec<BlockedPeerView>` with peer info

### Blocklist Subscriptions

#### `subscribe_blocklist`
- **Does**: Subscribes to external blocklist from another peer
- **Fields**: blocklist_id, maintainer_peer_id, name, auto_apply

#### `unsubscribe_blocklist`
- **Does**: Removes blocklist subscription

#### `list_blocklists`
- **Does**: Lists subscribed blocklists with entry counts

#### `add_blocklist_entry` / `remove_blocklist_entry`
- **Does**: Manages entries in user's own blocklists
- **Use case**: Sharing block lists with other users

### IP Blocking

#### `block_ip`
- **Does**: Blocks IP address or CIDR range
- **Validates**: Valid IP or network format

#### `unblock_ip`
- **Does**: Removes IP block

## Data Types

### `BlockedPeerView`
- **Fields**: peer_id, peer_username, peer_alias, reason, blocked_at

### `BlocklistSubscriptionView`
- **Fields**: id, maintainer_peer_id, name, description, auto_apply, entry_count

### `BlocklistEntryView`
- **Fields**: peer_id, reason, added_at, added_by

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api.rs` | All blocking methods | Method signature changes |
| `network/ingest.rs` | `is_blocked` check before accepting messages | Return type change |
| `network/events.rs` | `is_ip_blocked` for connection filtering | Method removal |

## Blocking Hierarchy

1. **Direct blocks**: Explicit user action, highest priority
2. **Blocklist blocks**: From subscribed lists with `auto_apply=true`
3. **IP blocks**: Network-level, affects all peers from that IP

## Notes
- Blocklists are shareable between users (community moderation)
- `auto_apply` controls whether blocklist entries are automatically enforced
- IP blocking supports both individual IPs and CIDR notation
- Blocking is local-only; doesn't propagate to network
- Posts from blocked peers hidden but not deleted
