# tasks.rs

## Purpose
Spawns background threads for async API operations. Each function takes an `ApiClient` and `Sender<AppMessage>`, performs work off the UI thread, and sends results back via the message channel.

## Components

### Thread Operations
- `load_threads` - Fetches thread list for catalog
- `load_thread` - Fetches single thread details (with peer download fallback)
- `create_thread` - Creates new thread with optional files
- `create_post` - Creates post with attachments (sequential: post → uploads → messages)

### File Operations
- `download_image` - Downloads and decodes image to `LoadedImage`
- `download_text_file` - Downloads text file content
- `download_media_file` - Downloads video/audio bytes
- `download_pdf_file` - Downloads PDF bytes
- `save_file` - Saves bytes to user-chosen location

### Social Operations
- `load_identity` - Fetches local peer identity
- `load_peers` - Fetches followed peers list
- `add_peer` - Adds peer via friend code
- `unfollow_peer` - Removes peer from following

### DM Operations
- `load_conversations` - Fetches DM conversation list
- `load_messages` - Fetches messages for a conversation
- `send_dm` - Sends encrypted DM

### Import Operations
- `import_fourchan` - Imports 4chan thread via backend (with topic selection)
- `import_reddit` - Imports Reddit thread via backend (with topic selection)
- `refresh_thread_source` - Re-fetches an imported thread from its source URL for new posts

### UI Helpers
- `pick_files` - Opens native file picker dialog

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` | Functions accept `ApiClient` + `Sender<AppMessage>` | Signature changes |
| `messages.rs` | Functions send correct message variants | Wrong message type |
| `api.rs` | API methods available on `ApiClient` | Method removal |

## Pattern

All task functions follow this pattern:
```rust
pub fn task_name(client: ApiClient, tx: Sender<AppMessage>, args...) {
    thread::spawn(move || {
        let result = client.api_method(args);
        if tx.send(AppMessage::ResultVariant(result)).is_err() {
            error!("failed to send message");
        }
    });
}
```

## Thread Loading

`load_thread` has special behavior:
1. For initial load: Try `download_thread` first (fetches from peers)
2. If download fails: Fall back to `get_thread` (local only)
3. For refresh (`is_refresh=true`): Skip peer download, use local only

## Post Creation Flow

1. Create post via API (returns post without attachments)
2. Upload each attachment file sequentially
3. Send `PostCreated` message (post visible immediately)
4. Send `PostAttachmentsLoaded` message (attachments appear)

This allows posts to appear quickly while uploads complete in background.

## Notes
- All operations run in `std::thread::spawn` (not tokio)
- Uses blocking reqwest client
- Errors sent as `Err` variants, handled by message processor
- File picker uses `rfd` crate for native dialogs
