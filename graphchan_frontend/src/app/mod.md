# mod.rs (app module)

## Purpose
Core application module containing the main `GraphchanApp` struct and orchestrating all UI state, API communication, and rendering. This is the central hub that ties together all frontend functionality.

## Components

### `GraphchanApp`
- **Does**: Main application struct implementing `eframe::App`
- **Interacts with**: All UI modules, API client, message channels
- **Key fields**: `api`, `tx`/`rx` (message channel), `view` (current ViewState), `threads`, `peers`, image caches

### `FileType`
- **Does**: Enum classifying files by MIME type for appropriate handling
- **Interacts with**: File viewers, attachment rendering
- **Variants**: Image, Video, Audio, Text, Pdf, Archive, Other

### `FileViewerState` / `FileDownloadState`
- **Does**: Track state of file downloads and viewer windows
- **Interacts with**: `file_viewers`, `file_downloads` HashMaps

### `get_video_cache_dir`
- **Does**: Returns path to video cache directory (~/.graphchan/cache/videos)
- **Interacts with**: Video player initialization

### Image Pipeline
- **Does**: Manages async image loading with rate limiting
- **Interacts with**: `image_textures`, `image_loading`, `image_pending`, `image_errors`, `download_queue`
- **Constant**: `MAX_CONCURRENT_DOWNLOADS = 4`

## Submodules

- **messages** - `AppMessage` enum and handlers
- **state** - All UI state structs
- **tasks** - Background task spawning
- **ui** - All UI rendering modules

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `GraphchanApp::new(cc)` constructor | Constructor signature |
| All UI modules | `GraphchanApp` fields accessible | Field removal |
| `messages.rs` | `process_messages()` method | Method removal |

## App Lifecycle

1. `GraphchanApp::new(cc)` - Initialize state, load identity, start refresh
2. `update(ctx, frame)` - Called each frame:
   - Process incoming messages via `rx`
   - Render current view based on `ViewState`
   - Handle keyboard input
3. Background tasks communicate via `tx` channel

## Key State Fields

| Field | Purpose |
|-------|---------|
| `view` | Current view (Catalog, Thread, Settings, etc.) |
| `threads` | Cached thread list for catalog |
| `peers` | Known peers by ID |
| `image_textures` | Loaded image textures by file ID |
| `download_queue` | Pending image downloads (rate limited) |
| `audio_device` | SDL2 audio for video playback |

## Notes
- Uses `std::sync::mpsc` for thread-safe message passing
- Image downloads rate-limited to 4 concurrent to avoid overwhelming backend
- Audio device initialized once and shared across all video players
- Context stored in `ctx` field for background task repaints
