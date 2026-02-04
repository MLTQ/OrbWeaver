# messages.rs

## Purpose
Defines the `AppMessage` enum and implements message handling for async communication between background tasks and the UI thread. All API responses flow through this channel.

## Components

### `AppMessage`
- **Does**: Enum of all possible async messages from background tasks
- **Interacts with**: `tasks.rs` sends messages, `GraphchanApp` receives and processes
- **Pattern**: Each variant wraps `Result<T, anyhow::Error>` for error handling

### Message Categories

**Thread Operations**
- `ThreadsLoaded` - Catalog refresh complete
- `ThreadLoaded` - Single thread details loaded
- `ThreadCreated` - New thread created
- `PostCreated` - New post created

**File Operations**
- `ImageLoaded` - Image downloaded and decoded
- `TextFileLoaded` - Text file content loaded
- `MediaFileLoaded` - Video/audio bytes loaded
- `PdfFileLoaded` - PDF bytes loaded
- `FileSaved` - File saved to disk
- `UploadProgress` - Upload progress update

**Social Operations**
- `IdentityLoaded` - Local identity fetched
- `PeersLoaded` - Peer list loaded
- `PeerAdded` - New peer followed
- `ConversationsLoaded` - DM conversation list
- `MessagesLoaded` - Messages in a conversation
- `DmSent` - DM sent successfully

**Reactions**
- `ReactionsLoaded` - Reactions for a post
- `ReactionAdded` / `ReactionRemoved` - Reaction updates

**Blocking**
- `BlockedPeersLoaded` - Blocked peer list
- `PeerBlocked` / `PeerUnblocked` - Block status changes
- `BlocklistsLoaded` - Subscribed blocklists

**Import**
- `ImportFinished` - Thread import complete
- `ImportError` - Import failed

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `tasks.rs` | All message variants constructible | Variant removal |
| `mod.rs` | `process_messages()` handles all variants | Missing handler |

## Message Processing

`GraphchanApp::process_messages()` runs each frame:
```rust
while let Ok(msg) = self.rx.try_recv() {
    match msg {
        AppMessage::ThreadsLoaded(result) => { ... }
        AppMessage::ImageLoaded { file_id, result } => { ... }
        // ... handlers for each variant
    }
}
```

## Error Handling

- Success: Update relevant state, trigger UI refresh
- Error: Store in appropriate `*_error` field, show to user
- Errors logged via `log::error!` for debugging

## Notes
- Messages processed non-blocking via `try_recv()`
- Context repaint requested after state changes
- Graph layouts rebuilt when thread content changes
- Download queue processed after each image load completes
