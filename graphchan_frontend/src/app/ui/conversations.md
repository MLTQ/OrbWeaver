# conversations.rs

## Purpose
Renders the direct message conversation views: the conversations list with a "New Message" peer picker, and the single-conversation message thread with compose input.

## Components

### `render_conversations_list`
- **Does**: Renders the Messages page — header with "New Message" button, optional peer picker, and scrollable list of existing conversations
- **Interacts with**: `DmState` (conversations, picker state), `app.peers` (for friend list), `open_conversation` / `open_conversation_with_peer`
- **Peer picker**: Toggled by "New Message" button. Shows filterable list of friends from `app.peers`. Clicking a friend opens a conversation via `open_conversation_with_peer`. Peers with existing conversations are labelled "(existing)".

### `render_conversation`
- **Does**: Renders a single DM conversation with a specific peer
- **Interacts with**: `ConversationState`, `GraphchanApp.spawn_send_dm`
- **Layout**: Back button, peer info header, scrollable message list, compose input

### Message Display
- **Does**: Renders each message as a bubble with alignment based on direction
- **Interacts with**: `ConversationState.messages`
- **Visual**: Outgoing messages indented right with code_bg_color, incoming indented left with extreme_bg_color

### Compose Input
- **Does**: Multiline text input with Send button
- **Interacts with**: `state.new_message_body`, `spawn_send_dm`
- **Behavior**: Enter sends (without Shift), Shift+Enter for newline

### `open_conversation` (private)
- **Does**: Opens a conversation by peer_id, looks up peer info from `app.peers`
- **Interacts with**: `ConversationState`, `spawn_load_messages`, `ViewState::Conversation`

### `open_conversation_with_peer` (public)
- **Does**: Opens a conversation with a known `PeerView` (called from friends.rs and peer picker)
- **Interacts with**: `ConversationState`, `spawn_load_messages`, `ViewState::Conversation`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_conversations_list(app, ui, conversations)` for `ViewState::Messages` | Signature change |
| `mod.rs` (app) | `render_conversation(app, ui, state)` for `ViewState::Conversation` | Signature change |
| `friends.rs` | `open_conversation_with_peer(app, peer)` | Function removal/rename |
| `state.rs` | `DmState` has `show_new_conversation_picker`, `new_conversation_filter` | Field removal |
| `state.rs` | `ConversationState` has `peer_id`, `messages`, `new_message_body` | Field removal |

## Layout

```
Messages                              [+ New Message]
─────────────────────────────────────────────────────
(peer picker, if open:)
┌─ Select a friend ─────────────────────────────────┐
│ [Filter friends...]                                │
│ alice_p2p                              [Message]   │
│ bob_node  (existing)                   [Message]   │
└────────────────────────────────────────────────────┘

┌ alice_p2p ────────────────────────────────────────┐
│ hey, check out this thread           2 unread     │
│ 2026-02-05 14:30                                  │
└───────────────────────────────────────────────────┘
```

## Notes
- Peer picker filters friends in real-time by username/alias
- Peers with existing conversations show "(existing)" but can still be selected (opens the existing thread)
- Empty state ("No conversations yet") now shows guidance to use the "New Message" button
- Messages stick to bottom (newest visible) via `stick_to_bottom(true)`
- Read receipts shown as "✓ Read" when `read_at` is present
