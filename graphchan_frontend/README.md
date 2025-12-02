# Graphchan Frontend

This crate hosts an egui/eframe desktop UI for the Graphchan backend with support for multiple view modes, rich file attachment viewing (images, videos, text, markdown, PDFs), and real-time peer networking.

## Features

- **Four View Modes**: Posts list, force-directed graph, hierarchical (Sugiyama) graph, and chronological timeline
- **Rich Media Support**: Videos with audio/volume control, images with click-to-enlarge, markdown rendering
- **File Attachments**: Upload and view images, videos, text, markdown, PDFs
- **Keyboard Navigation**: Tab/Shift+Tab to cycle posts, Arrow keys to navigate graph structure
- **Multi-Parent Threading**: Reply to multiple posts simultaneously
- **4chan Import**: Archive and discuss 4chan threads
- **Identity System**: Embedded GPG-based identities (no external GPG required)
- **P2P Networking**: Direct peer connections via iroh
- **Portable**: Statically linked SDL2 and FFmpeg for easy distribution

## Architecture

- **App State** (`app/mod.rs`): Core application state, file viewers, attachment rendering, SDL2 audio
- **UI Modules** (`app/ui/`): Specialized views (catalog, thread, graph, sugiyama, chronological, images, dialogs, drawer)
- **Message System** (`app/messages.rs`): Async message handling for API responses and downloads
- **Task Spawning** (`app/tasks.rs`): Background threads for downloads, API calls, file operations
- **API Client** (`api.rs`): Typed REST client with endpoint helpers
- **Models** (`models.rs`): Serde structs matching backend JSON (ThreadSummary, PostView, FileResponse)
- **Importer** (`importer.rs`): 4chan thread fetching and conversion

## Getting Started

**Prerequisites:**
- None! SDL2 and FFmpeg are statically linked.

**Running the app:**

1. **Desktop bundle (recommended):**
   ```bash
   cargo run --bin graphchan_desktop
   ```
   Starts backend and GUI together, automatically configured.

2. **Standalone frontend:**
   ```bash
   # Terminal 1: Start backend
   cargo run --bin graphchan_backend -- serve

   # Terminal 2: Start frontend
   cargo run -p graphchan_frontend
   ```
   Enter backend URL in "API Base URL" field (default: http://127.0.0.1:8080)

## Usage

### Creating Threads
1. Click "Create Thread" button
2. Enter title and optional body text
3. Optionally attach files (images, videos, PDFs, etc.)
4. Click "Create" to publish

### Viewing Threads
- **Posts View**: Traditional list with full post content and inline attachments
- **Graph View**: Force-directed layout showing reply relationships
- **Sugiyama View**: Hierarchical layout emphasizing thread depth and structure
- **Timeline View**: Chronological bins showing temporal patterns

### Keyboard Navigation
- **Tab / Shift+Tab**: Cycle through posts in linear order (creation time)
- **Arrow Keys**: Navigate between parent/child posts in Graph/Sugiyama views
- **Enter**: Open selected post details or expand attachments
- **Esc**: Deselect post

### Working with Attachments
- **Images**: Click thumbnail to open full-size popup window
- **Videos**: Click to open player with audio controls and volume slider
- **Text/Markdown**: Click to open in scrollable viewer with rich formatting
- **PDFs**: Click to open save dialog

### Importing 4chan Threads
1. Click hamburger menu → "Import 4chan Thread"
2. Enter 4chan thread URL (e.g., `https://boards.4chan.org/g/thread/12345`)
3. Click "Import" and wait for completion

## Roadmap

**Completed:**
- Thread catalog and detail views
- Four view modes (Posts/Graph/Sugiyama/Timeline)
- Multi-file attachments with upload
- Image/video/text/markdown/PDF viewers
- Video audio playback with SDL2 (Static)
- Click-to-enlarge images
- 4chan thread importer
- Embedded GPG identity system (Sequoia)
- P2P peer networking
- Keyboard navigation

**Planned:**
- Full PDF page rendering
- Pagination/search in catalog
- Draft post autosave
- Attachment drag-and-drop
- Video thumbnail generation

## Documentation

For detailed architecture and implementation notes, see:
- `../Docs/Frontend.md` - Complete frontend documentation
- `../walkthrough.md` - Recent changes and technical decisions
- `../TIMELINE_GUIDE.md` - Timeline view details
- `../CHRONOLOGICAL_LAYOUT.md` - Chronological layout algorithm
