# Graphchan Frontend

This crate hosts an egui/eframe desktop UI for the Graphchan backend. It mirrors the layout exploration in `p2pchan`, but consumes the live REST API exposed by `graphchan_backend`.

## Architecture Sketch
- **App State** (`app`): drives view transitions between catalog and thread details, holds cached data, and ferries background request results back to the UI via channel messages.
- **API Client** (`api`): thin wrapper over `reqwest::blocking::Client` with typed helpers for `/threads`, `/threads/{id}`, `/threads/{id}/posts`, and file listings.
- **Models** (`models`): serde-compatible structs matching backend JSON payloads (threads, posts, files, and request bodies).
- **Views** (`app` for now): egui components for the thread catalog, thread detail pane, composer dialogs, and future importer/test utilities.
- **Importer Hooks** (planned): space reserved for the 4chan DAG importer lifted from the legacy `p2pchan` project.

## Next Steps
1. Harden the desk UI with pagination, search, and attachment previews.
2. Reintroduce graph layout experimentation & importer once core flows are stable.
3. Package the app alongside the backend for an integrated developer experience.

The layout/UX follows the prior `p2pchan` design notes (`Docs/gui_design/main_window.md`).

## Getting Started
1. Launch the backend (`cargo run --bin graphchan_backend`) so the REST API is available at the expected address.
2. In this crate run `cargo run` (or open the binary) to start the egui desktop client.
3. Use the toolbar to point the UI at any alternate endpoint (apply updates triggers a refresh).

## Roadmap Snapshot
- **Completed**
  - Thread catalog list with backend-backed data.
  - Thread detail view with reply composer, attachment listing, and immediate update on publish.
  - Configurable API base URL and basic status banner messaging.
  - 4chan importer dialog that recreates threads through the backend REST API.
- **Planned**
  - File attachment previews and downloads.
  - Pagination/search in the catalog view.
  - Reintroduction of the 4chan DAG importer and graph visualisation experiments.
  - Packaging scripts that launch backend + frontend together for dev testing.

For a deeper dive into architecture and future plans see `../Docs/Frontend.md`.
