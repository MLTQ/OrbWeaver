# Graphchan Frontend Documentation

## Overview
The `graphchan_frontend` crate is a desktop GUI implemented with `egui`/`eframe`. It provides a catalog and thread view for the Graphchan network, backed by the REST API served from `graphchan_backend`.

## Repository Layout
```
graphchan_frontend/
├── src/
│   ├── main.rs        # eframe entry point / window bootstrap
│   ├── app.rs         # UI state machine, egui widgets, channel handling
│   ├── api.rs         # Blocking reqwest client for backend endpoints
│   └── models.rs      # Serde models mirroring backend JSON payloads
└── README.md          # Quick-start instructions and roadmap snapshot
```

## Runtime Behaviour
- The app launches with a configurable API base URL (defaults to `http://127.0.0.1:8080`).
- A background worker thread is spawned for each API action to keep the egui event loop responsive.
- Messages flow back to the UI via an `mpsc::channel`, updating the catalog or thread views as responses arrive.
- Users can:
  - Refresh the thread catalog.
  - Create new threads (title + optional body).
  - Open a thread to see posts and publish replies.
  - Dismiss informational banners when updates complete.

## Integration with the Backend
- Thread and post payloads match the backend’s `ThreadSummary`, `ThreadDetails`, and `PostView` structures.
- Create payloads (`CreateThreadInput`, `CreatePostInput`) are serialized to the REST API as defined in `graphchan_backend/src/api.rs`.
- File APIs are wrapped in the client but not surfaced yet in the UI; the groundwork allows future attachment previews and downloads with minimal changes.

## Build & Run
1. Ensure the backend is running locally (`cargo run --bin graphchan_backend`).
2. From `graphchan_frontend/`, run `cargo run` to launch the GUI.
3. Use the toolbar to adjust the API base URL if the backend is listening elsewhere.

## Roadmap
### Completed
- `ApiClient` wrapper with endpoint helpers for threads, posts, and file metadata.
- Catalog view that lists threads and allows navigation into a detail pane.
- Thread detail UI with reply composer, attachment metadata, and inline updates after posting.
- Configurable API base URL with banner feedback when actions succeed or fail.
- Import dialog that pulls a 4chan thread and recreates it through the backend REST API.

### In Progress / Planned
- Richer attachment handling (thumbnails, download progress, inline previews).
- Pagination/search and better sorting for the catalog.
- Graph/DAG layout experiments (ported from the legacy `p2pchan` implementation).
  - Visible reply edges now render between posts in graph mode (one edge per post for now; multi-parent rendering still WIP).
- Improved error reporting (toast notifications, retry affordances).
- Packaging scripts to coordinate backend & frontend launch for operators.

## References
- Prior design notes: `p2pchan/docs/gui_design/main_window.md`, `p2pchan/docs/Post_Layout_Plan.md`.
- Backend API definition: `graphchan_backend/src/api.rs` and `Docs/Architecture.md`.
