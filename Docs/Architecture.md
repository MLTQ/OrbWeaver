# Graphchan Backend Architecture

## Goals
- Deliver a pure Rust backend that exposes a stable REST API while synchronising data over Iroh 0.94.0.
- Keep nodes fully self-contained: every runtime artifact lives next to the executable, so multiple instances can run on one machine without interfering with each other.
- Separate concerns cleanly (bootstrap, identity, persistence, domain logic, networking, presentation) to support iterative development and testing.

## Runtime Layout
```
graphchan_backend/
├── target/                   # Cargo build output
├── data/
│   └── graphchan.db          # SQLite database (auto-created)
├── blobs/                    # iroh-blobs FsStore state
├── files/
│   ├── uploads/              # Original on-disk copies of local uploads
│   └── downloads/            # Materialised downloads from peers
├── keys/
│   ├── iroh.key              # Stored as base64 JSON
│   └── gpg/
│       ├── private.asc
│       └── public.asc
└── logs/
```
All paths are resolved via `GraphchanPaths`, derived from `current_exe().parent()`. Launching the binary from different directories produces isolated nodes that only discover each other over the network.

## Module Layout
- `bootstrap`: Creates runtime directories, opens the database, applies migrations, and ensures the local identity exists. Inserts/updates the corresponding rows in `node_identity` and `peers`.
- `config`: Loads configuration from the environment (`GRAPHCHAN_API_PORT`, `GRAPHCHAN_PUBLIC_ADDRS`, `GRAPHCHAN_RELAY_URL`) and materialises the filesystem layout.
- `identity`: Manages GPG key generation (`gpg --homedir keys/gpg`), persists the Iroh secret key, and encodes/decodes friendcodes (`version:1`, `{peer_id, gpg_fingerprint, addresses[]}`).
- `database`: Embeds migrations and exposes rusqlite-based repository traits for threads, posts, peers, and files.
- `threading`: Implements thread CRUD and post insertion, including DAG edge management and view models consumed by the API.
- `files`: Streams uploads into the local `FsStore`, keeps path metadata in SQLite, mirrors bytes under `files/uploads/`, and persists blob tickets/checksums for redistribution.
- `peers`: Wraps friendcode registration, maintains peer metadata, and synthesises the local peer record on demand.
- `network`: Starts an Iroh router that hosts both the Graphchan gossip protocol and the `/iroh-bytes/4` blobs protocol, tracks live connections, serialises outbound gossip events, and helps dial friendcodes.
- `api`: Defines the Axum router and handlers for health, thread/post/file operations, and peer management. Emits networking events after local mutations.
- `cli`: Hosts the interactive shell and the HTTP server entrypoint. The shell lets operators manage friendcodes, inspect threads, post messages, and transfer attachments (`upload`, `download`) without a REST client. Online/offline status is inferred from active Iroh connections.
- `utils`: Houses shared helpers such as time utilities.

## Identity & Provisioning Flow
1. `graphchan_backend serve` loads configuration via `GraphchanConfig::from_env`.
2. `bootstrap::initialize` ensures directories exist and connects to SQLite.
3. Migrations execute on first run, and schema drift (e.g., new columns) is handled idempotently.
4. GPG keys are generated if missing; the fingerprint is written to `keys/gpg/fingerprint.txt` and exported to `public.asc`/`private.asc`.
5. An Iroh secret key is generated (versioned JSON) if `keys/iroh.key` is absent.
6. A friendcode combining `{peer_id, gpg_fingerprint, advertised addresses}` is produced and stored in both `node_identity` and the local `peers` row.
7. The Iroh endpoint boots (respecting relay configuration) and the Axum HTTP server starts listening.

## Networking Strategy
- **Transport**: A single Iroh router per node that accepts both the custom `graphchan/0` gossip ALPN and `/iroh-bytes/4` for blob transfers. If `GRAPHCHAN_RELAY_URL` is set, the underlying endpoint binds with `RelayMode::Custom` so traffic is routed via the configured relay.
- **Sessions**: Incoming and outgoing QUIC connections are wrapped in `ConnectionEntry` records kept in an `RwLock`. Connection lifetime is monitored and entries are pruned when streams close.
- **Events**: A Tokio `mpsc` channel buffers outbound `NetworkEvent`s. Each event is framed as `EventEnvelope {version, topic, payload}` (JSON) and dispatched over fresh unidirectional streams managed by the router. Payloads cover thread snapshots, post updates, `FileAvailable` announcements (with blob tickets), and on-demand requests/chunks for legacy peers.
- **Friendcodes**: The API decodes friendcodes into `EndpointAddr` values and instructs the network layer to dial the peer immediately. Addresses may be socket addresses or relay URLs.
- **Next steps**: Harden ticket redemption with retries/deduping and extend the ingest worker to fetch blobs automatically when tickets arrive.

## Database Schema Highlights
- `threads(id, title, creator_peer_id, created_at, pinned)`
- `posts(id, thread_id, author_peer_id, body, created_at, updated_at)`
- `post_relationships(parent_id, child_id)`
- `files(id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket)`
- `peers(id, alias, friendcode, iroh_peer_id, gpg_fingerprint, last_seen, trust_state)`
- `node_identity(id=1, gpg_fingerprint, iroh_peer_id, friendcode)`
- `settings(key, value)`
Indices cover frequent lookups (`posts.thread_id`, `post_relationships.child_id`, `files.post_id`).

## API Surface
The Axum router exposes:
- `/health`
- `/threads` (GET, POST)
- `/threads/{id}` (GET)
- `/threads/{id}/posts` (POST)
- `/posts/{id}/files` (GET, POST multipart)
- `/files/{id}` (GET streaming download)
- `/peers` (GET, POST friendcode)
- `/peers/self` (GET)
Handlers construct services on demand, perform validation, persist through repositories, then emit network events to notify peers. Incoming gossip is applied asynchronously via the ingest worker, which upserts threads/posts/files into SQLite so the REST surface reflects remote changes.

## Request Lifecycle Example
1. Client `POST /threads` with title/body.
2. `ThreadService` validates input, writes thread (and optional seed post) via repositories.
3. Response returns the materialised thread with posts.
4. Handler calls `NetworkHandle::publish_thread_snapshot`, which queues a gossip event for connected peers.

File uploads follow the same pattern with an added blob stage: bytes are written under `files/uploads/`, imported into the `FsStore`, metadata (including blob hash and ticket placeholder) is inserted, and a `FileAvailable` gossip message carrying a `BlobTicket` is queued. Peers missing the blob redeem the ticket via `/iroh-bytes/4`; legacy nodes still fall back to the `FileRequest`/`FileChunk` flow. Attachments returned by `/posts/{id}/files` now include a `ticket` field and a `present` flag so callers can decide whether to download or wait for replication.

## Known Gaps / Future Work
1. **Gossip robustness** – Add deduplication, retries/backoff, and telemetry around ingest failures.
2. **Blob redemption tests** – Add automated coverage that exercises ticket issuance, persistence, and redemption across multiple nodes.
3. **API polish** – Add pagination/search, richer error mapping, and authentication hooks.
4. **Observability** – Expand logging, metrics, and tracing to aid operators once nodes run long-lived.
5. **Integration testing** – Extend the ignored REST harness into a multi-node scenario that uploads via REST/CLI and verifies blob tickets replicate end to end.

## Documentation & Tooling
- Architectural intent lives here; design rationales and roadmap details are in `Docs/Design.md` and `Docs/ImplementationPlan.md`.
- `AGENTS.md` captures workflow guidance for contributors.
- Run `cargo fmt`, `cargo check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` before committing. Use `cargo run --bin graphchan_backend` to launch a development node.
