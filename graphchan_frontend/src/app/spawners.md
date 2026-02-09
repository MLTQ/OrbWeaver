# spawners.rs

## Purpose
Centralizes all background task spawning methods for `GraphchanApp`. Extracted from `mod.rs` to isolate the "fire async work" responsibility from core app state and rendering.

## Components

### Thread/Post Spawners
- `spawn_load_threads` - Loads thread catalog (guarded by `threads_loading` flag)
- `spawn_load_thread` - Loads full thread details (initial load)
- `spawn_refresh_thread` - Refreshes existing thread (smooth update, no reset)
- `spawn_create_thread` - Validates title, builds `CreateThreadInput`, submits
- `spawn_create_post` - Validates body, builds `CreatePostInput` with reply targets and attachments

### Import Spawners
- `spawn_import_fourchan` - Imports a 4chan thread by URL
- `spawn_import_reddit` - Imports a Reddit thread by URL (reads from unified `self.importer` state)

### Identity/Peer Spawners
- `spawn_load_identity` - Fetches local peer identity
- `spawn_load_peers` - Fetches known peers list

### Topic Spawners
- `spawn_load_topics` - Loads subscribed topics (guarded)
- `spawn_subscribe_topic` / `spawn_unsubscribe_topic` - Topic subscription management
- `spawn_load_theme_color` - Loads theme color from backend settings

### Feed Spawners
- `spawn_load_recent_posts` - Loads recent posts feed (guarded)
- `spawn_search` - Performs full-text search (spawns raw thread, not via `tasks`)

### DM Spawners
- `spawn_load_conversations` - Loads DM conversation list (guarded)
- `spawn_load_messages` - Loads messages for a conversation (guarded)
- `spawn_send_dm` - Sends a direct message

### Blocking Spawners
- `spawn_load_blocked_peers` / `spawn_block_peer` / `spawn_unblock_peer`
- `spawn_load_blocklists` / `spawn_subscribe_blocklist` / `spawn_unsubscribe_blocklist` / `spawn_load_blocklist_entries`
- `spawn_load_ip_blocks` / `spawn_load_ip_block_stats` / `spawn_add_ip_block` / `spawn_remove_ip_block`
- `spawn_import_ip_blocks` / `spawn_export_ip_blocks` / `spawn_clear_all_ip_blocks`

### Moderation Spawners
- `spawn_delete_thread` / `spawn_ignore_thread`
- `spawn_load_reactions` / `spawn_add_reaction` / `spawn_remove_reaction`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` | All `spawn_*` methods on `GraphchanApp` | Method removal/rename |
| `handlers_*.rs` | Spawners called to reload data after mutations | Missing spawner |
| UI modules | `spawn_*` methods callable from UI event handlers | Signature changes |

## Notes
- Most spawners delegate to `tasks::*` functions which do the actual async work
- Guard patterns (`if self.X_loading { return; }`) prevent duplicate in-flight requests
- `spawn_search` is the only spawner that uses raw `std::thread::spawn` instead of `tasks`
- All spawners use `self.api.clone()` and `self.tx.clone()` to pass to background tasks
