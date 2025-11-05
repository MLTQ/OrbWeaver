# Graphchan Backend Design

## Overview
Graphchan is a headless Rust service that backs the peer-to-peer Graphchan image board. Each node hosts an HTTP API, maintains its own SQLite state, and speaks to other nodes over Iroh 0.94.0. The binary is portable: runtime artifacts (database, uploaded files, cryptographic material, logs) live beside the executable so multiple nodes can run on the same machine without colliding.

Key goals:
- Provide a REST surface that frontends can consume without caring about the underlying peer-to-peer layer.
- Keep the codebase modular so database, networking, identity management, and domain logic evolve independently.
- Default to offline-first behaviour: serve local reads/writes immediately, then gossip updates when connectivity exists.

## Current Capabilities
- **Bootstrap** creates the required directory layout, runs migrations, and seeds the node’s identity (GPG + Iroh keypair).
- **Identity** shells out to `gpg` to produce signing/encryption keys, persists the Iroh secret key as base64 JSON, and emits friendcodes that bundle the Iroh peer ID, local GPG fingerprint, and optional advertised addresses.
- **Database** uses `rusqlite` with embedded migrations to store threads, posts (DAG via `post_relationships`), peers, and file metadata; repositories expose trait-based CRUD APIs for unit testing.
- **Domain services** (`ThreadService`, `PeerService`, `FileService`) provide validated business logic on top of the repositories; the file service now streams uploads into the on-disk `FsStore` while keeping SQLite metadata in sync.
- **HTTP API** is implemented with Axum and currently exposes `/health`, thread CRUD helpers, file upload/download endpoints, and peer management (including friendcode registration).
- **Networking** spins up an Iroh router (custom relay optional) that serves both the Graphchan gossip protocol and `/iroh-bytes/4`, tracks live QUIC connections, and broadcasts thread/file events to connected peers.

## Module Responsibilities
- `config`: Discovers filesystem layout via `GraphchanPaths` (derived from `current_exe().parent()`), loads port + network hints from environment (`GRAPHCHAN_API_PORT`, `GRAPHCHAN_PUBLIC_ADDRS`, `GRAPHCHAN_RELAY_URL`).
- `bootstrap`: Ensures directories/migrations exist and persists the local identity into `node_identity` and `peers` tables.
- `identity`: Manages GPG/Iroh key lifecycle, friendcode encoding/decoding (`version:1`, `{peer_id, gpg_fingerprint, addresses[]}`), and exposes `IdentitySummary` to the rest of the app.
- `database`: Holds models, migration SQL, and rusqlite-backed repository implementations used by the services.
- `threading`: Implements thread creation/listing and post insertion (including DAG edges) with validation helpers and view models.
- `files`: Streams uploads into the `FsStore`, mirrors bytes under `files/uploads/`, records metadata (including blob hash and ticket) in SQLite, and prepares streaming downloads from disk.
- `peers`: Wraps friendcode registration and keeps peer rows up to date, including synthesizing the local peer view from persisted identity data.
- `network`: Boots the Iroh router with Graphchan and blob protocols, maintains accepted/outgoing connections, and publishes JSON-framed envelopes over unidirectional streams; ingest applies updates and persists blob tickets.
- `api`: Defines the Axum router, request handlers, and error mapping; wires in services and the `NetworkHandle`.
- `cli`: Owns the top-level async runtime and currently delegates to the HTTP server.
- `utils`: Holds shared helpers (`now_utc_iso`, constants) used across modules.

## HTTP API Surface
All routes are served relative to the configured `GRAPHCHAN_API_PORT` (default `8080`).
- `GET /health` – Returns build version, identity summary, and known network addresses.
- `GET /threads?limit=N` – Lists recent threads (default 50, capped at 200).
- `POST /threads` – Creates a thread (optionally with an initial post).
- `GET /threads/{id}` – Retrieves thread details plus ordered posts.
- `POST /threads/{id}/posts` – Appends a post, recording parent relationships.
- `GET /posts/{id}/files` – Lists file metadata for a post.
- `POST /posts/{id}/files` – Accepts multipart upload (field `file`), saves to disk, and stores metadata.
- `GET /files/{id}` – Streams bytes from disk with disposition/content-type headers.
- `GET /peers` – Lists known peers (local + remote).
- `GET /peers/self` – Returns the synthetic local peer row.
- `POST /peers` – Registers a friendcode and attempts to connect over Iroh.

## Data Model Highlights
- `threads(id, title, creator_peer_id, created_at, pinned)`
- `posts(id, thread_id, author_peer_id, body, created_at, updated_at)`
- `post_relationships(parent_id, child_id)` implements the post DAG.
- `files(id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket)`
- `peers(id, alias, friendcode, iroh_peer_id, gpg_fingerprint, last_seen, trust_state)`
- `node_identity(id=1, gpg_fingerprint, iroh_peer_id, friendcode)`
- `settings(key, value)` for future tunables.

## Networking Design Snapshot
- Nodes run an Iroh router that accepts both the Graphchan gossip ALPN (`graphchan/0`) and `/iroh-bytes/4`; relay hints are optional and derived from config.
- Active connections are tracked in-memory. Outbound publish operations serialize `EventEnvelope {version, topic, payload}` to JSON and send them down new unidirectional streams.
- Event payloads cover thread snapshots, incremental post updates, file announcements (with optional tickets), request messages, and chunk responses. Gossip readers drain inbound streams and forward decoded envelopes to the ingest worker, which upserts data into SQLite, persists tickets, and requests blobs when necessary.
- Friendcodes are parsed into `EndpointAddr` values; the network layer attempts to dial peers immediately after a friendcode is registered via the API.

## File Handling
File uploads are persisted under `files/uploads/` with sanitized filenames while the bytes are simultaneously imported into the on-disk `FsStore`. Disk writes happen before the SQLite transaction commits the metadata, ensuring both the legacy path and the blob store are ready. Each upload records the `Blake3` hash (`blob_id`), issues a `BlobTicket`, enforces an optional `GRAPHCHAN_MAX_UPLOAD_BYTES` limit, and auto-detects MIME types via the `infer` crate. Attachments that violate the configured size cap are rejected early with a descriptive error.

Remote announcements arriving over gossip now carry optional tickets. The ingest worker upserts metadata, persists the ticket, and still falls back to the legacy `FileRequest`/`FileChunk` exchange when the blob store is missing the bytes. APIs and the CLI surface whether an attachment is present on disk (`present: true/false`) and expose the ticket so clients can decide whether to redeem it immediately. The REST endpoint `/posts/{id}/files` accepts filters such as `?missing_only=true` and `?mime=image/png` to support UI-side diagnostics.

### Interactive CLI
- Default entry point (`graphchan_backend cli`) launches an interactive shell for friendcode management, thread browsing, posting, and file transfer.
- Commands include `friendcode`, `add-friend`, `list-threads`, `view-thread`, `post`, `upload <thread_id> <path> [mime]`, `download <file_id> [dest]`, `check`, and `list-friends` (with online/offline status based on active Iroh connections).
- The CLI reuses the same services as the REST API, so all mutations go through the existing gossip pipeline automatically.

## Testing & Tooling
- Unit tests cover repository CRUD, thread/post flows, peer registration, and file persistence (using in-memory SQLite + temporary directories).
- The ignored integration harness in `tests/rest_api.rs` is reserved for end-to-end HTTP scenarios once networking behaviours stabilize.
- Recommended dev commands: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`, and `cargo run --bin graphchan_backend`.
- The executable now ships an interactive CLI (`graphchan_backend cli`) that prints your friendcode, lets you add peers, browse threads, post messages, and poll for new activity without hitting the REST API.

## Upcoming Work
Refer to `Docs/ImplementationPlan.md` for the detailed roadmap. The high-priority items are:
1. Harden gossip ingestion (dedupe, retries, telemetry) and surface metrics for replication health.
2. Add automated tests that cover ticket issuance, persistence, and redemption (CLI/REST multi-node scenarios).
3. Expand API surface (pagination, auth hooks) and polish peer discovery UX.
4. Build multi-node integration tests exercising friendcode exchange, gossip propagation, and blob replication end to end.
