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
- Added `NetworkHandle` that binds Iroh endpoint (custom relay support), maintains connection registry, accepts incoming connections, and attempts friendcode dialing.
- Gossip layer (`network/events.rs`) now serializes thread/post/file payloads, supports targeted direct sends, and keeps connection metadata so broadcasts and requests share the same worker.
- Ingest worker (`network/ingest.rs`) applies inbound snapshots/posts, tracks missing blobs, issues `FileRequest`s, and stores returned `FileChunk`s under `files/downloads/` with checksum verification.
- Added an interactive CLI (`graphchan_backend cli`) to manage friendcodes, inspect threads, and post messages without a REST client.
- Added documentation (`Docs/Architecture.md`, `Docs/NetworkingPlan.md`, `Docs/ImplementationPlan.md`) capturing current design decisions and roadmap.
- Unit tests (`cargo test`) pass; REST integration harness exists (ignored pending network sync).

## Next Actions
1. Gossip hardening:
   - Add deduplication/backoff, surface telemetry for queued/failed deliveries, and persist replication stats.
2. Encoding & signatures:
   - Switch to CBOR/MessagePack, add versioning + optional signatures to `EventEnvelope`.
3. Blob transport upgrade:
   - Replace the single-chunk transfer with the upstream Iroh blob API, support streaming for large attachments, and resume interrupted downloads.
4. Multi-node integration harness:
   - Update the ignored REST test (or add new one) to launch two nodes, exchange friendcodes, and verify posts/files replicate end to end.
5. REST/API polish:
   - Pagination/search/auth, better error mapping, and derived advertised addresses from the live endpoint rather than env hints.
