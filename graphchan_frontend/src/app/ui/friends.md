# friends.rs

## Purpose
Renders the peer/following management page where users can add peers via friend codes and manage their followed peers list. Central hub for social connections in the P2P network.

## Components

### `render_friends_page`
- **Does**: Main renderer with add-peer form and following list
- **Interacts with**: `GraphchanApp.peers`, `identity_state`, `tasks::add_peer`

### Add Peer Section
- **Does**: Text input for friend codes with Follow button
- **Interacts with**: `identity_state.friendcode_input`, `tasks::add_peer`
- **Supports**: Both short and legacy friend code formats

### Following List
- **Does**: Grid showing followed peers with actions
- **Interacts with**: `app.peers` (HashMap of PeerView)
- **Columns**: Username (clickable), Peer ID, Actions (Message, View Profile, Unfollow)

### Actions
- **Message**: Opens DM conversation via `ViewState::Conversation`
- **View Profile**: Opens identity drawer with `inspected_peer`
- **Unfollow**: Calls `tasks::unfollow_peer`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_friends_page(app, ui)` for `ViewState::Following` | Signature change |
| `tasks.rs` | `add_peer`, `unfollow_peer` functions available | Function removal |
| `drawer.rs` | `inspected_peer` set to view peer profile | Field type change |

## Layout

```
Following Management

┌─ Follow a Peer ─────────────────────────────┐
│ Enter a friend code to follow a peer:       │
│ (Supports both short and legacy formats)    │
│ [_________________________] [Follow]        │
└─────────────────────────────────────────────┘

Following
┌──────────┬──────────────────┬─────────────────────────┐
│ Username │ Peer ID          │ Actions                 │
├──────────┼──────────────────┼─────────────────────────┤
│ Alice    │ abc123...        │ 💬 Message │ View │ Unfollow │
└──────────┴──────────────────┴─────────────────────────┘
```

## Notes
- Username click navigates to `FollowingCatalog(peer)` to see that peer's threads
- Peer ID shown truncated in monospace for identification
- Adding peer is async with spinner feedback
