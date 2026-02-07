# mod.rs (app module)

## Purpose
Core application module containing the main `GraphchanApp` struct, constructor, `eframe::App` implementation, and top-level view routing. Acts as the central hub tying together all submodules. Kept lean after extracting spawners, file viewer logic, and message handlers into dedicated files.

## Components

### `GraphchanApp`
- **Does**: Main application struct implementing `eframe::App`
- **Interacts with**: All UI modules, API client, message channels
- **Key fields**: `api`, `tx`/`rx` (message channel), `view` (current ViewState), `threads`, `peers`, image caches

### `GraphchanApp::new`
- **Does**: Initializes all state, SDL2 audio, API client; fires initial data loads
- **Interacts with**: `spawners.rs` (calls ~10 `spawn_*` methods on startup)

### `eframe::App::update`
- **Does**: Per-frame loop: applies theme, handles keyboard input, processes messages, renders current view, manages auto-refresh and polling
- **Interacts with**: All UI modules via view routing, `messages::process_messages`

### URL Resolvers
- `resolve_download_url` / `resolve_blob_url` / `resolve_file_url` - Build full URLs from base URL and file/blob IDs
- **Interacts with**: `file_viewer.rs`, `handlers_threads.rs`, `handlers_files.rs`

### Navigation Helpers
- `execute_search` - Switches to SearchResults view and spawns search
- `open_thread_and_scroll_to_post` - Opens thread view with scroll target
- `set_reply_target` / `quote_post` - Reply targeting helpers

### `format_timestamp`
- **Does**: Parses RFC3339 timestamp to human-readable UTC string

## Extracted Submodules

The following were extracted from this file to reduce its size (1407 → 637 lines):

| Module | What was extracted |
|--------|--------------------|
| `spawners.rs` | All ~30 `spawn_*` methods |
| `file_viewer.rs` | `FileType`, `FileDownloadState`, `FileViewerState`, `FileViewerContent`, download queue, file rendering |
| `handlers_threads.rs` | Thread/post message handlers |
| `handlers_files.rs` | File/image message handlers |
| `handlers_blocking.rs` | Blocking/blocklist/IP block handlers |
| `handlers_misc.rs` | Import, identity, reaction, DM, search, topic, theme handlers |

## All Submodules

- **messages** - `AppMessage` enum and thin dispatcher
- **state** - All UI state structs
- **tasks** - Background task spawning (actual async work)
- **ui** - All UI rendering modules
- **file_viewer** - File type classification, viewers, download pipeline
- **spawners** - All `spawn_*` methods
- **handlers_threads** - Thread/post message handlers
- **handlers_files** - File/image message handlers
- **handlers_blocking** - Blocking message handlers
- **handlers_misc** - Remaining message handlers

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `GraphchanApp::new(cc)` constructor | Constructor signature |
| All UI modules | `GraphchanApp` fields accessible via `pub(super)` | Field removal |
| `messages.rs` | `process_messages()` method on `GraphchanApp` | Method removal |
| `file_viewer.rs` | Re-exported types: `FileType`, `FileDownloadState`, `FileViewerState`, `FileViewerContent` | Type removal |

## App Lifecycle

1. `GraphchanApp::new(cc)` - Initialize state, SDL2 audio, fire ~10 initial loads
2. `update(ctx, frame)` - Called each frame:
   - Apply theme if dirty
   - Handle keyboard input (before UI renders)
   - Process incoming messages via `process_messages()`
   - Auto-refresh threads (5s interval) and recent posts (configurable)
   - Route to current view renderer
   - Render overlay dialogs (create thread, import, topic manager, identity drawer, file viewers)
3. Background tasks communicate via `tx`/`rx` channel

## Notes
- Uses `std::sync::mpsc` for thread-safe message passing
- SDL2 audio device initialized once in constructor for video playback
- Theme reapplication gated by `theme_dirty` flag to avoid per-frame work
- View routing uses string-based type matching to avoid double-borrow issues with `self`
