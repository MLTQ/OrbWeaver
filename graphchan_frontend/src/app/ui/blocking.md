# blocking.rs

## Purpose
Renders the privacy and moderation page with tabbed interface for managing blocked peers, blocklist subscriptions, and IP blocks. Accessible via "Blocking" button in the top navigation bar.

## Components

### `render_blocking_page`
- **Does**: Main renderer with tab navigation between blocking features
- **Interacts with**: `BlockingState`, various spawn functions for API calls
- **Tabs**: Blocked Peers (0), Blocklists (1), IP Blocks (2)

### `render_blocked_peers_tab`
- **Does**: Two-column layout: blocked peers list with export | block form + CSV import
- **Interacts with**: `state.blocked_peers`, `spawn_block_peer`, `spawn_export_peer_blocks`, `spawn_import_peer_blocks`
- **Features**: List with unblock buttons, export CSV, import CSV, block by peer ID

### `render_blocklists_tab`
- **Does**: Shows subscribed blocklists with entry counts
- **Interacts with**: `state.blocklists`, blocklist subscription API
- **Features**: Subscribe to external blocklists, view entries, unsubscribe

### `render_ip_blocks_tab`
- **Does**: Manage IP-level blocks with export/import
- **Interacts with**: IP blocking API endpoints, `spawn_export_ip_blocks`, `spawn_import_ip_blocks`
- **Features**: Add IP/CIDR blocks, import/export text format, clear all, stats

### `render_blocked_peers_list`
- **Does**: Renders individual blocked peer entries with unblock action
- **Interacts with**: `spawn_unblock_peer`

### `render_blocklist_entries_dialog`
- **Does**: Modal window showing entries in a subscribed blocklist

### `render_clear_all_ip_blocks_dialog`
- **Does**: Confirmation dialog before clearing all IP blocks

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_blocking_page(app, ui, state)` for `ViewState::Blocking` | Signature change |
| `state.rs` | `BlockingState` has all peer/ip/blocklist state fields | Field removal |
| `spawners.rs` | Export/import peer/IP spawner functions | Function removal |
| `handlers_blocking.rs` | Handlers for PeerBlocksExported, PeerBlocksImported, IpBlocksExported | Handler removal |

## Layout

```
Catalog | Messages | New Thread | Import | Following | Topics | Blocking | Identity   [Settings]
─────────────────────────────────────────────────────────────────────────────────────────────────
← Back to Catalog    Privacy & Moderation
[Blocked Peers] [Blocklists] [IP Blocks]
─────────────────────────────────────────────

Tab 0: Blocked Peers
┌─ Blocked Peers ─────────┬─ Block a Peer ──────────────┐
│ peer1 [Unblock]         │ Peer ID: [____]             │
│ peer2 [Unblock]         │ Reason: [____]              │
│                         │ [Block Peer]                │
│ [Export CSV]            │                             │
│                         │ Import Blocked Peers        │
│ (export text shown      │ [CSV paste area]            │
│  inline when exported)  │ [Import]                    │
└─────────────────────────┴─────────────────────────────┘

Tab 2: IP Blocks
┌─ Blocked IPs ───────────┬─ Add IP Block ──────────────┐
│ Stats: Total | Active   │ IP/Range: [____]            │
│ 192.168.1.1  [Remove]   │ Reason: [____]              │
│ 41.0.0.0/8   [Remove]   │ [Block IP]                  │
│                         │                             │
│ [Clear All] [Export]    │ Import Blocklist             │
│                         │ [text paste area]           │
│ (export text shown      │ [Import]                    │
│  inline when exported)  │                             │
└─────────────────────────┴─────────────────────────────┘
```

## Notes
- Now accessible from main nav bar (Blocking button)
- Peer block export/import uses CSV format: `peer_id,reason,blocked_at`
- IP block export/import uses text format: `IP_OR_RANGE # reason`
- Export text shown inline with selectable text (no file dialog needed)
- Blocking is local-only; gossip broadcasts block actions for shared blocklists
