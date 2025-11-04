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
├── files/
│   ├── uploads/
│   └── downloads/
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
- `files`: Handles disk persistence for uploads, metadata storage, and download preparation. Blob IDs/checksums are stored but not yet populated.
- `peers`: Wraps friendcode registration, maintains peer metadata, and synthesises the local peer record on demand.
- `network`: Starts the Iroh endpoint, tracks live connections, serialises outbound gossip events, and exposes helpers for dialing friendcodes. Inbound event processing remains a TODO.
- `api`: Defines the Axum router and handlers for health, thread/post/file operations, and peer management. Emits networking events after local mutations.
- `cli`: Currently runs the HTTP server; future subcommands (e.g., key export) will attach here.
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
- **Transport**: A single Iroh endpoint per node with ALPN `graphchan/0`. If `GRAPHCHAN_RELAY_URL` is set, the endpoint binds with `RelayMode::Custom` so traffic is routed via the configured relay.
- **Sessions**: Incoming and outgoing QUIC connections are wrapped in `ConnectionEntry` records kept in an `RwLock`. Connection lifetime is monitored and entries are pruned when streams close.
- **Events**: A Tokio `mpsc` channel buffers outbound `NetworkEvent`s. Each event is framed as `EventEnvelope {version, topic, payload}` (JSON) and dispatched over fresh unidirectional streams. Current payloads cover thread and file notifications; receivers are not yet reading/acting on incoming streams.
- **Friendcodes**: The API decodes friendcodes into `EndpointAddr` values and instructs the network layer to dial the peer immediately. Addresses may be socket addresses or relay URLs.
- **Next steps**: Implement stream readers that parse inbound envelopes, validate them (GPG signatures are a future enhancement), and invoke the appropriate services to upsert posts/files.

## Database Schema Highlights
- `threads(id, title, creator_peer_id, created_at, pinned)`
- `posts(id, thread_id, author_peer_id, body, created_at, updated_at)`
- `post_relationships(parent_id, child_id)`
- `files(id, post_id, path, original_name, mime, blob_id, size_bytes, checksum)`
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

File uploads follow the same pattern: data is written to disk, metadata inserted, a future download path is reserved under `files/downloads/`, and a `FileAvailable` gossip message is queued. Peers missing the blob immediately issue a direct `FileRequest`; the owner answers with a `FileChunk` payload so the ingest worker can materialise the download and verify the announced checksum before exposing it through the REST API.

## Known Gaps / Future Work
1. **Gossip robustness** – Add deduplication, retries/backoff, and telemetry around ingest failures.
2. **Blob integration** – Populate `blob_id` + `checksum`, stage uploads in Iroh blobs, and fetch missing blobs when peers announce availability.
3. **API polish** – Add pagination/search, richer error mapping, and authentication hooks.
4. **Observability** – Expand logging, metrics, and tracing to aid operators once nodes run long-lived.
5. **Integration testing** – Expand the ignored REST harness into a multi-node scenario that exercises friendcode exchange, gossip, and file replication end to end.

## Documentation & Tooling
- Architectural intent lives here; design rationales and roadmap details are in `Docs/Design.md` and `Docs/ImplementationPlan.md`.
- `AGENTS.md` captures workflow guidance for contributors.
- Run `cargo fmt`, `cargo check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` before committing. Use `cargo run --bin graphchan_backend` to launch a development node.
