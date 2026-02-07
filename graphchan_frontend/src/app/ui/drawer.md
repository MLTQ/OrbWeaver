# drawer.rs

## Purpose
Renders the identity sidebar panel for viewing and editing user profiles. Handles both local identity management (own profile) and viewing other peers' profiles with follow/block actions.

## Components

### `render_identity_drawer`
- **Does**: Right-side sliding panel for identity management
- **Interacts with**: `IdentityState`, peer APIs, avatar upload
- **Controls**: `show_identity` boolean toggles visibility

### Peer Profile View (inspected_peer set)
- **Does**: Shows another peer's profile with follow/message/block actions
- **Interacts with**: `inspected_peer`, `tasks::add_peer`
- **Display**: Username, bio, friend code (copyable), GPG fingerprint

### Local Identity View (inspected_peer None)
- **Does**: Shows and edits own profile
- **Interacts with**: `local_peer`, avatar upload/crop
- **Editable**: Username, bio, avatar

### Avatar Cropper
- **Does**: Pan/zoom interface for cropping uploaded avatar images
- **Interacts with**: `AvatarCropperState`, file picker
- **Flow**: Pick image → crop/adjust → upload

### Friend Code Display
- **Does**: Shows short friend code with copy button
- **Interacts with**: `short_friendcode`, `friendcode` (full for copying)
- **Rationale**: Short code for display, full code copied (includes relay URLs for NAT traversal)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_identity_drawer(app, ctx)` called each frame | Signature change |
| `state.rs` | `IdentityState` has `local_peer`, `inspected_peer`, `cropper_state` | Field removal |
| `friends.rs` | Sets `inspected_peer` to view peer profile | Field type change |

## Layout

```
┌─ Identity Management ─────────────────┐
│                                       │
│ (Viewing peer profile)                │
│ ← Back to My Identity                 │
│                                       │
│ ┌─────────────────────────────────┐   │
│ │ Username: Alice                 │   │
│ │ Bio: Rust enthusiast            │   │
│ │ Friend Code: abc123...          │   │
│ │ [📋 Copy]                       │   │
│ │ Fingerprint: ABCD...            │   │
│ │                                 │   │
│ │ [Follow] [💬 Message] [Block]   │   │
│ └─────────────────────────────────┘   │
│                                       │
│ (Or own profile with edit fields)     │
└───────────────────────────────────────┘
```

## Notes
- Panel is resizable (default 300px width)
- Block action available for peers (IP block option)
- Avatar picker uses `rfd::FileDialog` for native file selection
- Peers without friend codes show explanation about gossip discovery
