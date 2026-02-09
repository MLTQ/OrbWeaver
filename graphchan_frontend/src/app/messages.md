# messages.rs

## Purpose
Defines the `AppMessage` enum for async communication and a thin `process_messages()` dispatcher that routes each message variant to the appropriate handler method. All API responses flow through this channel.

## Components

### `AppMessage`
- **Does**: Enum of all possible async messages from background tasks (~40 variants)
- **Interacts with**: `tasks.rs` sends messages, handler modules process them
- **Pattern**: Each variant wraps `Result<T, anyhow::Error>` for error handling

### `process_messages(app)`
- **Does**: Drains the message channel and dispatches each variant to the appropriate handler method
- **Interacts with**: All `handlers_*.rs` modules via method calls on `GraphchanApp`
- **Rationale**: Thin dispatcher keeps this file small; actual logic lives in handler modules

### Message Categories

**Thread Operations** → `handlers_threads.rs`
- `ThreadsLoaded`, `ThreadLoaded`, `ThreadCreated`, `PostCreated`, `PostAttachmentsLoaded`

**File Operations** → `handlers_files.rs`
- `ImageLoaded`, `TextFileLoaded`, `MediaFileLoaded`, `PdfFileLoaded`, `FileSaved`

**Import Operations** → `handlers_misc.rs`
- `ImportFinished`, `ThreadImported`, `ImportError`, `ThreadSourceRefreshed`

**Identity/Peer Operations** → `handlers_misc.rs`
- `IdentityLoaded`, `PeersLoaded`, `PeerAdded`, `AvatarUploaded`, `ProfileUpdated`, `ThreadFilesSelected`

**Reaction Operations** → `handlers_misc.rs`
- `ReactionsLoaded`, `ReactionAdded`, `ReactionRemoved`

**DM Operations** → `handlers_misc.rs`
- `ConversationsLoaded`, `MessagesLoaded`, `DmSent`

**Blocking Operations** → `handlers_blocking.rs`
- `BlockedPeersLoaded`, `PeerBlocked`, `PeerUnblocked`
- `BlocklistsLoaded`, `BlocklistSubscribed`, `BlocklistUnsubscribed`, `BlocklistEntriesLoaded`
- `IpBlocksLoaded`, `IpBlockStatsLoaded`, `IpBlockAdded`, `IpBlockRemoved`
- `IpBlocksImported`, `IpBlocksExported`, `IpBlocksCleared`
- `PeerIpBlocked`, `PeerIpBlockFailed`

**Search/Feed/Topics/Theme** → `handlers_misc.rs`
- `SearchCompleted`, `RecentPostsLoaded`, `TopicsLoaded`, `TopicSubscribed`, `TopicUnsubscribed`, `ThemeColorLoaded`

**Unhandled**
- `UploadProgress` - TODO: display upload progress in UI

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `tasks.rs` | All message variants constructible | Variant removal |
| `handlers_threads.rs` | Called for thread/post variants | Handler method removal |
| `handlers_files.rs` | Called for file variants | Handler method removal |
| `handlers_blocking.rs` | Called for blocking variants | Handler method removal |
| `handlers_misc.rs` | Called for remaining variants | Handler method removal |
| `mod.rs` | `process_messages(app)` function | Function removal |

## Notes
- Refactored from ~1031 lines to ~227 lines by extracting handler logic into dedicated modules
- Messages processed non-blocking via `try_recv()` in a `while` loop
- Each match arm is a single method call — no inline logic
- `UploadProgress` is the only unhandled variant (TODO)
