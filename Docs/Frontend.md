# Graphchan Frontend Documentation

## Overview
The `graphchan_frontend` crate is a desktop GUI implemented with `egui`/`eframe`. It provides a catalog and thread view for the Graphchan network, backed by the REST API served from `graphchan_backend`. Features include multiple view modes (List, Graph, Timeline, Hierarchical, and Radial), rich attachment viewing (images, videos, text, markdown, PDFs), and interactive post navigation.

## Repository Layout
```
graphchan_frontend/
├── src/
│   ├── main.rs           # eframe entry point / window bootstrap
│   ├── lib.rs            # Library entry with run_frontend functions
│   ├── app/
│   │   ├── mod.rs        # Core app state, file viewers, attachment rendering
│   │   ├── messages.rs   # AppMessage enum and async message handlers
│   │   ├── state.rs      # ViewState, ThreadState, and UI state structs
│   │   ├── tasks.rs      # Background task spawning (downloads, API calls)
│   │   └── ui/
│   │       ├── catalog.rs      # Thread catalog grid view
│   │       ├── thread.rs       # Posts list view with floating composer
│   │       ├── graph.rs        # Force-directed graph layout with physics
│   │       ├── chronological.rs # Timeline view with time binning
│   │       ├── sugiyama.rs     # Hierarchical layered graph layout
│   │       ├── radial.rs       # Radial phylogenetic tree layout
│   │       ├── node.rs         # Shared node rendering for graph views
│   │       ├── images.rs       # Image viewer popup windows
│   │       ├── dialogs.rs      # Create thread & import dialogs
│   │       ├── input.rs        # Keyboard navigation handlers
│   │       └── drawer.rs       # Identity sidebar & avatar editor
│   ├── api.rs            # ApiClient wrapper for backend REST endpoints
│   ├── models.rs         # Serde models (ThreadSummary, PostView, FileResponse)
│   └── importer.rs       # 4chan thread import logic
└── README.md             # Quick-start instructions
```

## Runtime Behaviour
- The app launches with a configurable API base URL (defaults to `http://127.0.0.1:8080`).
- A background worker thread is spawned for each API action to keep the egui event loop responsive.
- Messages flow back to the UI via an `mpsc::channel`, updating the catalog or thread views as responses arrive.
- SDL2 audio subsystem is initialized at startup to enable video playback with audio.
- Users can:
  - Browse threads in a grid catalog view
  - View threads in five modes: List, Graph, Timeline, Hierarchical, or Radial
  - Create new threads with title, body, and file attachments
  - Reply to posts with multi-parent support
  - Upload and view multiple file types (images, videos, text, markdown, PDFs)
  - Click images to enlarge in popup windows
  - Play videos with audio and volume control
  - Import 4chan threads for archival/discussion

## Integration with the Backend
- Thread and post payloads match the backend's `ThreadSummary`, `ThreadDetails`, and `PostView` structures.
- Create payloads (`CreateThreadInput`, `CreatePostInput`) are serialized to the REST API as defined in `graphchan_backend/src/api.rs`.
- File attachments use the backend's file storage system with proper `present` flag handling:
  - Backend's `get_thread` endpoint (api.rs:128) uses `ThreadService::with_file_paths()` to check file existence
  - Files are returned with `present: true/false` based on actual file availability on disk
  - Frontend uses `post.files` directly for rendering, ensuring correct attachment display across all views

## Build & Run
Two ways to start the UI:

1. **Desktop bundle (recommended)** – `cargo run --bin graphchan_desktop`. This starts the backend and GUI inside one process, automatically points the HTTP client at `http://127.0.0.1:{GRAPHCHAN_API_PORT}`, and still exposes the REST API for other tooling.
2. **Headless backend + standalone frontend** – run `cargo run --bin graphchan_backend -- serve` (or `-- cli`) and separately `cargo run -p graphchan_frontend`. Paste the daemon’s URL into the “API Base URL” field to connect to remote peers or other machines.

## Thread View Modes

The frontend offers five distinct ways to view and navigate threads, each with different node placement algorithms optimized for different use cases:

### 1. List View (thread.rs)
**Traditional linear discussion view**
- **Algorithm**: Posts displayed in chronological order (sorted by `created_at`)
- Posts rendered as full-width cards with complete content
- Floating draft composer window (bottom-right)
- Inline attachment rendering with file type icons
- Click images to enlarge, click other files to open in specialized viewers
- Multi-file attachment support per post
- Best for: Reading full conversations in order

### 2. Graph View (graph.rs)
**Force-directed physics-based layout**
- **Algorithm**: Spring-embedder physics simulation
  - Repulsive force between all node pairs (inverse square law)
  - Attractive spring force along reply edges (Hooke's law)
  - Configurable repulsion strength and desired edge length
  - Original post (OP) pinned at center as anchor
  - Velocity damping for stability (0.80 factor)
  - 5-second initial simulation, then settles
- Posts rendered as nodes with curved bezier edges
- Pan with right-click drag, zoom with scroll wheel
- Click posts to select (yellow highlight), neighbors highlighted
- Pin button on nodes to lock positions
- Best for: Exploring conversation structure organically

### 3. Timeline View (chronological.rs)
**Chronological layout with time binning**
- **Algorithm**: Temporal binning with vertical stacking
  - Posts grouped into time bins (configurable: 1 min - 24 hours)
  - Bin index = `(post_time - earliest_time) / bin_size`
  - X position = bin_index * bin_width
  - Posts within same bin stack vertically (Y offset by node height)
  - Time axis on left margin shows bin timestamps
- Posts rendered as compact cards
- Same pan/zoom controls as graph view
- Curved bezier edges connect replies across time
- Best for: Understanding temporal flow of conversation

### 4. Hierarchical View (sugiyama.rs)
**Layered directed graph layout (Sugiyama-style)**
- **Algorithm**: Layer-based hierarchical layout
  - Layer assignment: BFS from OP, each post on layer = max(parent layers) + 1
  - Posts sorted within layers to minimize edge crossings
  - X position based on layer, Y position distributed within layer
  - Handles multi-parent replies by using deepest parent's layer
- Posts arranged in clean horizontal layers
- Edges flow left-to-right showing reply depth
- Best for: Understanding conversation hierarchy and branching structure

### 5. Radial View (radial.rs)
**Radial phylogenetic tree layout**
- **Algorithm**: Ring-based depth assignment inspired by phylogenetic trees
  - Ring 0 (center): Original post (OP)
  - Ring N: Posts where `max(ring of each parent) = N - 1`
  - Multi-parent posts placed based on their DEEPEST parent
  - Posts distributed angularly within each ring by creation time
  - Angular position: `(index / count) * 2π - π/2` (starting from top)
- Concentric ring guides show depth levels
- Curved bezier edges connect posts to parents (curve toward center)
- Click post to center viewport on it
- Manual rotation buttons for exploring different angles
- Best for: Visualizing conversation evolution and reply depth

## View Comparison

| View | Layout Algorithm | Best For | Multi-Parent Handling |
|------|-----------------|----------|----------------------|
| List | Chronological sort | Reading full content | Shows all parent links |
| Graph | Force-directed physics | Organic exploration | Edges to all parents |
| Timeline | Time bins + stacking | Temporal analysis | Edges to all parents |
| Hierarchical | BFS layering | Structure clarity | Deepest parent determines layer |
| Radial | Ring-based depth | Evolution visualization | Deepest parent determines ring |

## Keyboard Navigation

The application supports full keyboard navigation for efficiency:

### Global Navigation
- **Tab**: Select next post (chronological order)
- **Shift+Tab**: Select previous post
- **Space**: Re-center viewport on selected post
- **Esc**: Deselect post

### Graph/Hierarchical/Timeline Views
- **Arrow Keys**: Navigate based on visual layout

### Radial View Navigation
- **Left/Right Arrow**: Move between posts on the same ring
- **Up Arrow**: Move to inner ring (toward center/OP)
- **Down Arrow**: Move to outer ring (away from center)
- Navigation finds the angularly closest post when changing rings

### VIM-Style Navigation (All Graph Views)
- **U/P**: Move cursor through parent posts
- **I**: Select parent at cursor (secondary selection)
- **J/;**: Move cursor through reply posts
- **K**: Select reply at cursor (secondary selection)
- **O/L**: Alternative keys for cursor movement

## File Attachment System

### Supported File Types

**Images** (`.png`, `.jpg`, `.gif`, etc.)
- Thumbnail previews in all views
- Click to open full-size popup window (images.rs:6-24)
- Auto-download and cache on first view
- Enlarge/shrink controls in viewer

**Videos** (`.mp4`, `.webm`, `.mov`, etc.)
- Inline video player using `egui-video` crate
- SDL2 audio playback with volume control slider
- Play/pause/seek controls via egui-video UI
- Volume persists across videos (stored in GraphchanApp.video_volume)
- Save button to export video file
- Implementation: mod.rs:619-647

**Text Files** (`.txt`, `.log`, etc.)
- Rendered in read-only code editor with syntax highlighting
- Scrollable viewer window
- Implementation: mod.rs:600-608

**Markdown Files** (`.md`, `.markdown`)
- Rich rendering using `egui_commonmark` library
- Supports headers, lists, code blocks, links, emphasis
- Auto-detected by file extension or MIME type
- Rendered with CommonMarkCache for performance
- Implementation: mod.rs:609-618, messages.rs:389-409

**PDF Files** (`.pdf`)
- Simplified viewer with save functionality
- Shows "PDF page rendering will be added in a future update"
- Save button to export PDF externally
- Implementation: mod.rs:649-663
- Note: Full page rendering planned for future (requires `pdfium-render` integration)

### File Viewer Architecture

**File Opening Flow:**
1. User clicks file in any view → `app.open_file_viewer()` called (mod.rs:489)
2. FileViewerState created with Loading content (mod.rs:493-501)
3. Background download task spawned based on file type (tasks.rs)
4. Download completes → AppMessage sent to UI thread
5. Message handler updates FileViewerState.content (messages.rs)
6. Viewer window renders content (mod.rs:576-690)

**Key Components:**
- `FileViewerState`: Tracks viewer state per file (mod.rs:119-126)
- `FileViewerContent`: Enum for different content types (Loading/Text/Markdown/Video/Pdf/Error)
- `open_file_viewer()`: Entry point for opening files (mod.rs:489-509)
- `render_file_viewers()`: Renders all open viewer windows (mod.rs:576-690)

### Video Audio System

**SDL2 Integration:**
- SDL2 audio subsystem initialized in `GraphchanApp::new()` (mod.rs:198-224)
- Creates `AudioDevice` that's passed to video players
- Graceful degradation: if SDL2 init fails, videos play without audio
- Error logging for debugging audio initialization issues
- **Static Linking**: SDL2 is statically linked on macOS/Linux/Windows, so no external installation is required.

**Volume Control:**
- Collapsible volume slider in video viewer (mod.rs:628-639)
- Range: 0% - 100% with percentage display
- Applied via `player.options.set_audio_volume(volume)`
- Volume persisted in `GraphchanApp.video_volume` field

**Player Creation:**
- Video files downloaded to cache directory first
- Player created with `egui_video::Player::new()` + `.with_audio(audio_device)`
- Initial volume applied on creation (messages.rs:420)
- Players stored in `FileViewerState` for inline rendering

## Attachment Rendering by View

### Posts List View (thread.rs:217-222)
- Uses `render_file_attachment()` for each file in `post.files`
- Images: thumbnails (150px max width) with click-to-enlarge
- Other files: clickable labels with file type icons
- All files render inline within post frame

### Graph View (graph.rs:589-641)
- Uses `render_node_attachments()` helper
- Images: 300px max thumbnails with click-to-enlarge
- Other files: clickable labels with MIME type detection
- Attachment rendering scales with zoom level
- Files sourced from `post.files` (not `state.attachments`)

### Timeline View (chronological.rs:635-737)
- Uses `render_node_attachments()` helper (same as graph but different sizing)
- Images: 120px max thumbnails with click-to-enlarge
- Other files: icon + filename clickable labels (🎥📄📕📎)
- All sizing scales with zoom factor
- Files sourced from `post.files` for correct `present` flag

### Image Preview Helper (graph.rs:628-662)
**Shared utility for graph and timeline views**
- Returns `ImagePreview` enum: Ready/Loading/Error/None
- Handles download triggering via `spawn_download_image()`
- Uses `resolve_download_url()` to build correct URLs
- Manages image loading states (`image_loading`, `image_pending`, `image_textures`)

## Async Message Flow

**Message Types (messages.rs:11-57):**
- `ThreadsLoaded`, `ThreadLoaded` - Thread data from API
- `PostCreated`, `PostAttachmentsLoaded` - Post creation responses
- `ImageLoaded`, `TextFileLoaded`, `MediaFileLoaded`, `PdfFileLoaded` - File downloads
- `IdentityLoaded`, `PeersLoaded` - Identity/peer data
- `ThreadFilesSelected` - File picker results

**Message Processing:**
- `GraphchanApp::process_messages()` runs every frame (mod.rs:241-258)
- Drains channel with `try_recv()` loop
- Each message handler updates app state and triggers UI updates
- Download completion triggers next queued download (mod.rs:356-361)

**Post Creation with Attachments (tasks.rs:42-91):**
1. Create post via API (returns post without files)
2. Upload each attachment file sequentially
3. Send `PostCreated` message (post appears in UI without attachments)
4. Send `PostAttachmentsLoaded` message if any uploaded (attachments appear)
5. This creates a brief window where post shows without attachments, then updates

## Key Dependencies

**UI Framework:**
- `eframe` 0.29 - egui application framework with native window support
- `egui_extras` 0.29 - Additional widgets (image support)

**Media Handling:**
- `egui-video` 0.9 - Video player with seek/play/pause controls
- `sdl2` 0.37 - Audio subsystem for video playback (Statically linked)
- `image` 0.25 - Image loading/decoding (PNG, JPEG, GIF)
- `egui_commonmark` 0.18 - Markdown rendering with rich formatting

**Network/API:**
- `reqwest` 0.12 - HTTP client with rustls TLS
- `poll-promise` 0.3 - Async task management for egui

**Utilities:**
- `rfd` 0.14 - Native file picker dialogs
- `open` 5 - Open files/URLs in system default apps
- `html2text` 0.4 - HTML to plain text conversion (for imports)

## Troubleshooting

**Video audio not playing:**
- Ensure SDL2 is installed on your system (macOS: `brew install sdl2`)
- Check logs for "SDL2 audio initialized successfully"
- If init fails, videos still play but without audio

**Images showing "builder error":**
- Usually caused by malformed download URLs
- Check `resolve_download_url()` is used instead of manual URL construction
- Verify backend's `download_url` field is properly populated

**Attachments showing as "(remote)":**
- Backend's `get_thread` must use `ThreadService::with_file_paths()` (not `.new()`)
- Ensures `FileView.present` is set based on actual file existence
- Frontend uses `post.files` directly (not `state.attachments`)

**Images not clickable:**
- Image widgets must use `.sense(egui::Sense::click())`
- Click handler inserts into `app.image_viewers` map
- `render_image_viewers()` must be called in main update loop

## References
- Prior design notes: `p2pchan/docs/gui_design/main_window.md`, `p2pchan/docs/Post_Layout_Plan.md`.
- Backend API definition: `graphchan_backend/src/api.rs` and `Docs/Architecture.md`.
- Timeline view documentation: `TIMELINE_GUIDE.md`
- Chronological layout documentation: `CHRONOLOGICAL_LAYOUT.md`
