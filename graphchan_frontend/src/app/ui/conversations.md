# conversations.rs

## Purpose
Renders the direct message conversation view for peer-to-peer encrypted messaging. Displays message history with a peer and provides a compose interface for sending new messages.

## Components

### `render_conversation`
- **Does**: Main renderer for a DM conversation with a specific peer
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

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_conversation(app, ui, state)` called for `ViewState::Conversation` | Signature change |
| `state.rs` | `ConversationState` has `peer_id`, `messages`, `new_message_body` | Field removal |
| `tasks.rs` | `spawn_send_dm(state)` sends message via API | Function removal |

## Layout

```
← Back to Private Threads    Direct Message with {peer_name}
─────────────────────────────────────────────────────────────
                                          [Outgoing message]
                                               timestamp ✓ Read
[Incoming message]
timestamp
─────────────────────────────────────────────────────────────
[Type a message...                              ] [Send]
```

## Notes
- Messages stick to bottom (newest visible) via `stick_to_bottom(true)`
- Read receipts shown as "✓ Read" when `read_at` is present
- Back button returns to Catalog (not Messages list) - may be intentional for quick return to main view
