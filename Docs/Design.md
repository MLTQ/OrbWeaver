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
- **Domain services** (`ThreadService`, `PeerService`, `FileService`) provide validated business logic on top of the repositories and expose serializable view models to the API.
- **HTTP API** is implemented with Axum and currently exposes `/health`, thread CRUD helpers, file upload/download endpoints, and peer management (including friendcode registration).
- **Networking** spins up an Iroh endpoint (custom relay optional), tracks live QUIC connections, and can broadcast thread/file events to connected peers. Event ingestion remains stubbed until gossip integration lands.

## Module Responsibilities
- `config`: Discovers filesystem layout via `GraphchanPaths` (derived from `current_exe().parent()`), loads port + network hints from environment (`GRAPHCHAN_API_PORT`, `GRAPHCHAN_PUBLIC_ADDRS`, `GRAPHCHAN_RELAY_URL`).
- `bootstrap`: Ensures directories/migrations exist and persists the local identity into `node_identity` and `peers` tables.
- `identity`: Manages GPG/Iroh key lifecycle, friendcode encoding/decoding (`version:1`, `{peer_id, gpg_fingerprint, addresses[]}`), and exposes `IdentitySummary` to the rest of the app.
- `database`: Holds models, migration SQL, and rusqlite-backed repository implementations used by the services.
- `threading`: Implements thread creation/listing and post insertion (including DAG edges) with validation helpers and view models.
- `files`: Persists uploaded blobs under `files/uploads/`, records metadata (size, original name, MIME, checksum placeholder) in SQLite, and prepares streaming downloads from disk.
- `peers`: Wraps friendcode registration and keeps peer rows up to date, including synthesizing the local peer view from persisted identity data.
- `network`: Boots the Iroh 0.94.0 endpoint, maintains accepted/outgoing connections, and publishes JSON-framed envelopes over unidirectional streams. Inbound handlers are still TODO.
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
- `files(id, post_id, path, original_name, mime, blob_id, size_bytes, checksum)`
- `peers(id, alias, friendcode, iroh_peer_id, gpg_fingerprint, last_seen, trust_state)`
- `node_identity(id=1, gpg_fingerprint, iroh_peer_id, friendcode)`
- `settings(key, value)` for future tunables.

## Networking Design Snapshot
- Nodes bind an Iroh endpoint with ALPN `graphchan/0`; relay hints are optional and derived from config.
- Active connections are tracked in-memory. Outbound publish operations serialize `EventEnvelope {version, topic, payload}` to JSON and send them down new unidirectional streams.
- Event payloads cover thread snapshots, incremental post updates, file announcements, request messages, and chunk responses. Gossip readers drain inbound streams and forward decoded envelopes to the ingest worker, which upserts data into SQLite and requests blobs when necessary.
- Friendcodes are parsed into `EndpointAddr` values; the network layer attempts to dial peers immediately after a friendcode is registered via the API.

## File Handling
File uploads are persisted under `files/uploads/` with sanitized filenames that retain the original extension when provided. Disk writes happen before the database transaction commits the metadata, ensuring the on-disk copy exists when the file row is created. Each upload records a Blake3 checksum and derived `blob_id`, so metadata can be verified after replication. `FileService::prepare_download` validates disk presence before streaming; missing files yield a `404` from the API.

Remote announcements arriving over gossip create or update file rows under `files/downloads/`. If the blob is missing or mismatched, the ingest worker automatically issues a direct `FileRequest` to the announcing peer; the owner responds with a `FileChunk` payload carrying the file bytes, which are written into the downloads directory and verified against the advertised checksum.

## Testing & Tooling
- Unit tests cover repository CRUD, thread/post flows, peer registration, and file persistence (using in-memory SQLite + temporary directories).
- The ignored integration harness in `tests/rest_api.rs` is reserved for end-to-end HTTP scenarios once networking behaviours stabilize.
- Recommended dev commands: `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`, and `cargo run --bin graphchan_backend`.

## Upcoming Work
Refer to `Docs/ImplementationPlan.md` for the detailed roadmap. The high-priority items are:
1. Harden gossip ingestion (dedupe, retries, telemetry) and surface metrics for replication health.
2. Promote the current in-process blob exchange to the Iroh blob API once it stabilises, adding chunking/streaming for large attachments.
3. Expand API surface (pagination, auth hooks) and polish peer discovery UX.
4. Build multi-node integration tests exercising friendcode exchange, gossip propagation, and file replication end to end.
