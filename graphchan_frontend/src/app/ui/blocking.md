# blocking.rs

## Purpose
Renders the privacy and moderation page with tabbed interface for managing blocked peers, blocklist subscriptions, and IP blocks. Provides user-controlled content filtering for the P2P network.

## Components

### `render_blocking_page`
- **Does**: Main renderer with tab navigation between blocking features
- **Interacts with**: `BlockingState`, various spawn functions for API calls
- **Tabs**: Blocked Peers (0), Blocklists (1), IP Blocks (2)

### `render_blocked_peers_tab`
- **Does**: Two-column layout showing blocked peers list and block form
- **Interacts with**: `state.blocked_peers`, `spawn_block_peer`
- **Features**: List with unblock buttons, form to block by peer ID with optional reason

### `render_blocklists_tab`
- **Does**: Shows subscribed blocklists with entry counts
- **Interacts with**: `state.blocklists`, blocklist subscription API
- **Features**: Subscribe to external blocklists, view entries, unsubscribe

### `render_ip_blocks_tab`
- **Does**: Manage IP-level blocks (for node operators)
- **Interacts with**: IP blocking API endpoints

### `render_blocked_peers_list`
- **Does**: Renders individual blocked peer entries with unblock action
- **Interacts with**: `spawn_unblock_peer`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_blocking_page(app, ui, state)` for `ViewState::Blocking` | Signature change |
| `state.rs` | `BlockingState` has `current_tab`, `blocked_peers`, `blocklists` | Field removal |
| `tasks.rs` | Block/unblock peer functions, blocklist functions | Function removal |

## Layout

```
← Back to Catalog    Privacy & Moderation
─────────────────────────────────────────────
[Blocked Peers] [Blocklists] [IP Blocks]
─────────────────────────────────────────────

Tab 0: Blocked Peers
┌─ Blocked Peers ─────┬─ Subscribed Blocklists ─┐
│ peer1 [Unblock]     │ blocklist1 (42 entries) │
│ peer2 [Unblock]     │ [Unsubscribe]           │
│                     │                         │
│ Block a Peer        │ Subscribe to Blocklist  │
│ Peer ID: [____]     │ URL: [____]             │
│ Reason: [____]      │ [Subscribe]             │
│ [Block Peer]        │                         │
└─────────────────────┴─────────────────────────┘
```

## Notes
- Blocklists are external lists of peers to block (community moderation)
- IP blocks likely for node operators to ban abusive connections
- Blocking is local-only; doesn't affect other peers' views
