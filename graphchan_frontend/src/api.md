# api.rs

## Purpose
HTTP client wrapper for communicating with the Graphchan backend REST API. Provides typed methods for all API endpoints with proper error handling and timeout configuration.

## Components

### `ApiClient`
- **Does**: Wraps reqwest clients with base URL and convenience methods
- **Interacts with**: All `tasks.rs` functions, backend REST API
- **Fields**: `base_url`, `client` (30s timeout), `upload_client` (1hr timeout)

### Shared Clients
- `SHARED_CLIENT` - Standard HTTP client with 30s timeout
- `UPLOAD_CLIENT` - File upload client with 1hr timeout
- **Rationale**: Large file uploads need extended timeouts

### `get_shared_client` / `get_upload_client`
- **Does**: Lazily initializes static HTTP clients
- **Interacts with**: `OnceLock` for thread-safe initialization

### `sanitize_base_url`
- **Does**: Validates and normalizes base URL (adds http://, removes trailing slash)
- **Interacts with**: `ApiClient::new`, `set_base_url`

## API Methods

### Threads
- `list_threads()` → `Vec<ThreadSummary>`
- `get_thread(id)` → `ThreadDetails`
- `download_thread(id)` → `ThreadDetails` (triggers peer download)
- `create_thread(input, files)` → `ThreadDetails`

### Posts
- `list_recent_posts(limit)` → `RecentPostsResponse`
- `create_post(thread_id, input)` → `PostView`
- `get_post_attachments(post_id)` → `Vec<FileResponse>`

### Files
- `upload_file(post_id, path)` → `FileResponse`
- `download_file_bytes(url)` → `Vec<u8>`

### Identity & Peers
- `get_identity()` → `PeerView`
- `list_peers()` → `Vec<PeerView>`
- `add_peer(friendcode)` → `PeerView`
- `unfollow_peer(peer_id)` → `()`

### DMs
- `list_conversations()` → `Vec<ConversationView>`
- `get_messages(peer_id)` → `Vec<DirectMessageView>`
- `send_dm(to_peer_id, body)` → `DirectMessageView`

### Blocking
- `list_blocked_peers()` → `Vec<BlockedPeerView>`
- `block_peer(peer_id, reason)` → `BlockedPeerView`
- `unblock_peer(peer_id)` → `()`

### Topics
- `list_subscribed_topics()` → `Vec<String>`
- `subscribe_topic(topic_id)` → `()`
- `unsubscribe_topic(topic_id)` → `()`

### Import & Refresh
- `import_thread(url, topics)` → `String` (thread ID) — imports with optional topic tags
- `refresh_thread(thread_id)` → `ThreadDetails` — re-fetches imported thread from source for new posts

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `tasks.rs` | All API methods available | Method removal/rename |
| `models.rs` | Response types match API | Type mismatches |

## Error Handling

- Uses `anyhow::Result<T>` for all fallible operations
- HTTP errors converted via `error_for_status()?`
- Context added via `.context("description")` for debugging

## Notes
- Base URL can be changed at runtime via `set_base_url`
- All methods are blocking (used from background threads)
- User-Agent set to "GraphchanFrontend/0.1"
- Uses `reqwest::blocking::Client` (not async)
