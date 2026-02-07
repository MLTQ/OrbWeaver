# ingest.rs

## Purpose
Processes inbound gossip messages, validating and storing them in the database. Handles thread sync, file downloads, profile updates, and reaction propagation with deduplication and blocking checks.

## Components

### `run_ingest_loop`
- **Does**: Main message processing loop
- **Interacts with**: Database, FsStore, Endpoint, IpBlockChecker
- **Features**: Deduplication via `seen_messages`, auto-resync on hash mismatch

### `handle_message`
- **Does**: Dispatches message to appropriate handler by payload type
- **Returns**: `Option<ResyncRequest>` if thread needs re-download

## Message Handlers

### ThreadAnnouncement
- **Does**: Stores thread metadata, optionally triggers download
- **Flow**: Check if new → Store stub → Download if hash differs
- **Dedup**: `thread:{id}` in seen_messages

### PostUpdate
- **Does**: Upserts post record, creates stub peer if needed
- **Validates**: Thread exists, not from blocked peer
- **Dedup**: `post:{id}`

### FileAvailable
- **Does**: Stores file metadata, downloads blob via ticket
- **Flow**: Store record → Download blob → Export to downloads dir
- **Handles**: FileAvailable before PostUpdate (deferred download)

### ProfileUpdate
- **Does**: Updates peer profile (username, bio, avatar)
- **Validates**: Not from blocked peer
- **Dedup**: `profile:{peer_id}:{timestamp}`

### ReactionUpdate
- **Does**: Adds/removes reaction with signature verification
- **Validates**: Signature matches reactor, not blocked
- **Dedup**: `reaction:{post_id}:{emoji}:{reactor}`

## Helper Functions

### `download_thread_snapshot_blob`
- **Does**: Downloads ThreadDetails blob and ingests all posts/files
- **Flow**: Fetch blob → Deserialize → Upsert thread → Upsert each post

### `ensure_stub_peer`
- **Does**: Creates minimal peer record if unknown author
- **Sets**: `trust_state = "unknown"`, minimal fields
- **Rationale**: Posts may arrive before profile updates

### `download_and_store_file`
- **Does**: Downloads blob via ticket, exports to filesystem
- **Interacts with**: FsStore downloader, file export

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `network.rs` | `run_ingest_loop` signature | Parameter changes |
| `events.rs` | All payload types handled | Missing handler |
| `blocking.rs` | `is_blocked` check available | Return type change |

## Race Condition Handling

| Scenario | Solution |
|----------|----------|
| FileAvailable before PostUpdate | Store file record, defer download until post exists |
| Post from unknown peer | Create stub peer with `trust_state: "unknown"` |
| Duplicate messages | `seen_messages` HashSet deduplication |
| Hash mismatch | Trigger ResyncRequest for full thread download |

## Notes
- All operations are idempotent (upsert semantics)
- Blocked peer check happens early to avoid unnecessary processing
- Auto-resync spawns background task to avoid blocking ingest loop
- File downloads use blob ticket for content-addressed retrieval
