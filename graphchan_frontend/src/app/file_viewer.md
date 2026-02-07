# file_viewer.rs

## Purpose
File type classification, viewer state management, download pipeline (rate-limited image queue), and file viewer window rendering. Extracted from `mod.rs` to isolate all file/media handling from core app logic.

## Components

### `FileType`
- **Does**: Enum classifying files by MIME type (Image, Video, Audio, Text, Pdf, Archive, Other)
- **Interacts with**: `render_file_attachment`, `open_file_viewer` for dispatch logic
- **Methods**: `from_mime(mime)` for classification, `icon()` for display

### `FileDownloadState`
- **Does**: Tracks download progress (progress percentage, byte counts)
- **Interacts with**: `file_downloads` HashMap on `GraphchanApp`

### `FileViewerState`
- **Does**: Holds state for an open file viewer window (file ID, name, MIME, content, markdown cache)
- **Interacts with**: `file_viewers` HashMap, `render_file_viewers`

### `FileViewerContent`
- **Does**: Enum of loaded content variants (Loading, Text, Markdown, Video, Pdf, Error)
- **Interacts with**: `handlers_files.rs` sets content, `render_file_viewers` renders it

### `get_video_cache_dir()`
- **Does**: Returns `~/.graphchan/cache/videos`, creating it if needed
- **Interacts with**: `download_media_file`, `handle_media_file_loaded` in `handlers_files.rs`

### `format_file_size(bytes)`
- **Does**: Human-readable file size formatting (KB/MB/GB)

### Download Queue (on `GraphchanApp`)
- `spawn_download_image` - Enqueues an image download
- `spawn_load_image` - Public alias for `spawn_download_image`
- `process_download_queue` - Starts downloads up to `MAX_CONCURRENT_DOWNLOADS` (4)
- `on_download_complete` - Decrements counter, processes next queued item

### File Rendering (on `GraphchanApp`)
- `render_file_attachment` - Entry point: dispatches to image or generic renderer
- `render_image_attachment` - Shows thumbnail with click-to-zoom; triggers lazy download
- `render_generic_file` - Shows icon + clickable filename + size + context menu (save/copy link)

### File Viewer Lifecycle (on `GraphchanApp`)
- `open_file_viewer` - Creates `FileViewerState`, dispatches download by type
- `save_file_as` - Triggers save-as dialog via `tasks::save_file_as`
- `download_text_file` / `download_media_file` / `download_pdf_file` / `download_generic_file` - Type-specific download initiators
- `render_file_viewers` - Renders all open viewer windows (text editor, markdown, video player, PDF placeholder)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` | Re-exports `FileType`, `FileDownloadState`, `FileViewerState`, `FileViewerContent` | Type removal/rename |
| `handlers_files.rs` | `get_video_cache_dir()`, `FileViewerContent` variants | Variant changes |
| `handlers_threads.rs` | `spawn_load_image`, `spawn_download_image` | Method removal |
| UI thread/catalog | `render_file_attachment` for post attachments | Signature changes |
| `mod.rs` update loop | `render_file_viewers(ctx)` called each frame | Method removal |

## Notes
- `MAX_CONCURRENT_DOWNLOADS = 4` prevents overwhelming the backend with parallel image fetches
- Video files are cached to disk at `~/.graphchan/cache/videos/{file_id}.mp4` for replay without re-download
- `download_media_file` checks cache first before initiating network download
- PDF viewer is a placeholder (save-to-disk only); full rendering planned via `pdfium-render`
- `FileViewerContent::Video` wraps `egui_video::Player` which doesn't implement `Debug`
