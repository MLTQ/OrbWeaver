# handlers_files.rs

## Purpose
Handles all async message responses related to file and image loading. Called by the dispatcher in `messages.rs` when file-related `AppMessage` variants arrive.

## Components

### `handle_image_loaded`
- **Does**: Stores decoded image in `image_pending` (or error in `image_errors`), advances download queue
- **Interacts with**: `image_loading`, `image_pending`, `image_errors`, `on_download_complete` in `file_viewer.rs`

### `handle_text_file_loaded`
- **Does**: Sets file viewer content to `Text` or `Markdown` (detected by extension/MIME)
- **Interacts with**: `file_viewers`, `FileViewerContent`, `CommonMarkCache`
- **Rationale**: Markdown detection enables rich rendering via `egui_commonmark`

### `handle_media_file_loaded`
- **Does**: Caches video bytes to disk, creates `egui_video::Player` with audio support
- **Interacts with**: `get_video_cache_dir`, `audio_device`, `video_volume`, `FileViewerContent::Video`
- **Rationale**: Disk caching avoids re-downloading on replay; audio device shared across players

### `handle_pdf_file_loaded`
- **Does**: Stores raw PDF bytes in viewer content
- **Interacts with**: `FileViewerContent::Pdf`

### `handle_file_saved`
- **Does**: Shows success/failure banner for file save operations
- **Interacts with**: `info_banner`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `messages.rs` | All 5 handler methods exist on `GraphchanApp` | Method removal/rename |
| `file_viewer.rs` | `FileViewerContent` variants, `get_video_cache_dir` | Variant/function changes |

## Notes
- `handle_media_file_loaded` falls back to player-without-audio if `audio_device` is `None`
- Video cache path: `~/.graphchan/cache/videos/{file_id}.mp4`
- Markdown detection uses `.md`, `.markdown` extensions and `markdown` in MIME type
