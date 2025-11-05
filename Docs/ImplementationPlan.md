# Implementation Plan

## Major Milestones
1. **Foundational Backend**
   - Scaffold Rust workspace, config/bootstrap modules, and SQLite schema.
   - Implement identity provisioning (GPG + Iroh key generation) per `Docs/Iroh Docs 0.94.0`.
   - Expose initial REST API (threads/posts/peers, `/health`).
2. **Storage & Files**
   - Build structured repository layer and domain services.
   - Implement file service (disk persistence, metadata, download prep) and REST endpoints.
   - Validate functionality with unit tests and a local REST harness.
3. **Networking Integration**
   - Bind Iroh endpoint (Graphchan ALPN), honor relay/public addr config.
   - Create gossip event pipeline (publish/subscribe, connection manager).
   - Propagate thread/post/file events across peers; replicate blobs via Iroh.
4. **Replication & Frontend Support**
   - End-to-end multi-node integration harness (friendcode exchange, post/file sync).
   - Add pagination/search/auth to REST API; document usage.
   - Prepare for production (observability, tuning, operator docs).

## Fine-Grained Roadmap
1. **Schema & Config**
   - Centralize migrations and config loaders (`GraphchanConfig`, `GraphchanPaths`, `NetworkConfig`).
   - Provide deterministic constructors for tests; keep env overrides.
2. **Identity & Bootstrap**
   - Shell out to `gpg --quick-generate-key` / `--quick-add-key` (ed25519 + cv25519).
   - Serialize Iroh secret key (base64 JSON) via `iroh_base::SecretKey`.
   - Persist `node_identity` + auto-register local peer row.
3. **Database Repositories**
   - Implement thread/post/peer/file CRUD with rusqlite + in-memory tests.
   - Support DAG edges (`post_relationships`) and file metadata (`original_name`).
4. **Domain Services**
   - `ThreadService`: list/get/create threads, insert posts, emit view models.
   - `PeerService`: upsert/list peers, handle friendcodes.
   - `FileService`: upload to disk, list metadata, prepare downloads.
5. **REST API**
   - Axum state (config, identity, database, network).
   - Routes: `/health`, `/threads`, `/threads/{id}`, `/threads/{id}/posts`, `/posts/{id}/files`, `/files/{id}`, `/peers`, `/peers/self`.
   - Stream downloads with proper headers; handle multipart uploads.
6. **Networking**
   - `NetworkHandle`: load secret, bind endpoint, optional relay map.
   - Event loop (`network/events.rs`) with unidirectional message dispatch.
   - Connection registry (accept loop & outbound friendcode dial); stubs log gossip TODOs.
7. **Integration Testing**
   - Ignored REST harness in `tests/rest_api.rs` for local verification.
   - Plan for multi-node friendcode simulation once network gossip is live.
8. **Remaining Work**
   - Implement gossip framing (deserialize inbound streams, apply updates).
   - Blob staging/fetch via Iroh; update file service schema (blob hash, checksum verification).
   - Multi-node integration + CLI friendcode UX.
   - Observability and API polish (pagination, auth, metrics).

## Accomplishments to Date
- Bootstrapped project structure, config, identity provisioning (GPG + Iroh keys) and persisted node metadata.
- Implemented repository layer with migrations for threads, posts, peers, files (including DAG edges) and added unit coverage.
- Built `ThreadService`, `PeerService`, and `FileService`; exposed REST endpoints for threads/posts/files/peers; added multipart upload + stream download handling.
- Established Axum server with shared state, JSON health output, and integration hooks to `FileService`/`ThreadService`.
- **Migrated to iroh-gossip 0.94.0** - Replaced custom gossip protocol with production-ready topic-based pub/sub using Plumtree/HyParView algorithms.
- Gossip layer now uses `iroh_gossip::net::Gossip` with topic subscriptions, automatic neighbor management, and built-in message deduplication.
- Ingest worker (`network/ingest.rs`) applies inbound snapshots/posts, tracks missing blobs, issues `FileRequest`s, and stores returned `FileChunk`s under `files/downloads/` with checksum verification.
- Added an interactive CLI (`graphchan_backend cli`) to manage friendcodes, inspect threads, and post messages without a REST client.
- File uploads now stream into the `FsStore`, retain MIME hints, enforce optional size caps (`GRAPHCHAN_MAX_UPLOAD_BYTES`), expose blob tickets/presence via API/CLI, and provide interactive `upload`/`download` commands for manual verification.
- Two-node integration harness (`two_node_gossip_replication`) proves friendcode exchange, bi-directional post replication, and attachment request/response flows end-to-end (blob ticket validation still manual).
- Added documentation (`Docs/Architecture.md`, `Docs/NetworkingPlan.md`, `Docs/ImplementationPlan.md`) capturing current design decisions and roadmap.
- Unit tests (`cargo test`) pass; REST integration harness exists (ignored pending network sync).
- **Frontend**: Fixed build errors, egui 0.27 API compatibility, and borrow checker issues in graphchan_frontend.

## Next Actions
1. **Frontend Enhancements**:
   - Add peer/friendcode management UI (display local friendcode, add peers dialog).
   - Implement persistent settings (save API URL to localStorage or config file).
   - Add image thumbnails for attachments using `egui_extras::image`.
   - Implement file upload UI (multipart form support).
   
2. **Gossip hardening**:
   - Add deduplication/backoff, surface telemetry for queued/failed deliveries, and persist replication stats.
   - Monitor iroh-gossip metrics for network health.
   
3. **Blob ticket validation**:
   - Add unit tests for `FileService` (store writes + `persist_ticket`), ingest ticket handling, and downloader happy-path coverage.
   - Extend the ignored REST harness to assert tickets appear in responses and a second node can redeem them.
   
4. **Encoding & signatures**:
   - Switch to CBOR/MessagePack for more efficient serialization.
   - Add versioning + optional GPG signatures to `EventEnvelope` for tamper detection.
   
5. **Multi-node integration harness**:
   - Automate a two-node scenario that exchanges friendcodes, posts files, and verifies blob replication end to end.
   
6. **REST/API polish**:
   - Pagination/search/auth, better error mapping.
   - Derive advertised addresses from the live endpoint rather than env hints.

## Frontend Roadmap

### Completed (November 2025)
- ✅ Fixed egui 0.27 API compatibility issues (`.margin()` → `.inner_margin()`).
- ✅ Resolved Rust borrow checker conflicts in dialog rendering and view state management.
- ✅ Basic thread catalog with create/view/reply functionality.
- ✅ File attachment display with download links.
- ✅ 4chan thread importer.
- ✅ Configurable API base URL.

### Quick Wins (Priority 1)
- **Peer Management UI**: 
  - Display local friendcode in a copyable text box.
  - "Add Peer" dialog to paste and register friendcodes.
  - Show online/offline status for known peers.
- **Persistent Settings**:
  - Save API URL to config file or browser storage equivalent.
  - Remember window size/position.
- **Image Thumbnails**:
  - Use `egui_extras::image` to show inline previews for image attachments.
  - Support common formats (PNG, JPEG, GIF).
- **File Upload UI**:
  - Multipart form support for attaching files to posts.
  - Drag-and-drop file upload area.
  - Progress indicators for upload/download.

### Medium Term (Priority 2)
- **DAG Visualization**:
  - Basic post relationship graph using parent_post_ids.
  - Indentation or tree view for reply chains.
  - Interactive node selection to highlight relationships.
- **Better Error Handling**:
  - Toast notifications instead of inline error text.
  - Retry mechanisms with exponential backoff.
  - Network status indicator.
- **Pagination & Search**:
  - Paginate thread catalog (currently loads all threads).
  - Search threads by title/content.
  - Filter by date, author, or tags.
- **Real-time Updates**:
  - Poll backend for new posts/threads.
  - Optional: WebSocket or SSE for push updates.

### Long Term (Priority 3)
- **Advanced Graph Layout** (port from p2pchan):
  - Force-directed graph for complex DAG visualization.
  - Zoomable/pannable canvas.
  - Collapse/expand sub-threads.
- **Rich Media Support**:
  - Video/audio playback.
  - PDF/document viewers.
  - Code syntax highlighting.
- **Multi-Window/Tabs**:
  - Open multiple threads in separate windows or tabs.
  - Split-pane view for catalog + thread.
- **Offline Mode**:
  - Cache threads/posts for offline viewing.
  - Queue posts to send when connection restored.
- **Theming & Accessibility**:
  - Dark/light mode toggle.
  - Font size customization.
  - Keyboard navigation.
