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

## Execution Modes
- `graphchan_desktop` – Bundled GUI that boots a `GraphchanNode`, serves the REST API on the configured port, and launches the egui frontend pointed at that local URL.
- `graphchan_backend serve` – Headless REST daemon for remote/front-end clients.
- `graphchan_backend cli` – Interactive shell for friendcodes, posting, and manual file transfer.

All three binaries share the same runtime directories, so copying any executable to a new folder yields an isolated node.

## Module Layout
- `bootstrap`: Creates runtime directories, opens the database, applies migrations, and ensures the local identity exists. Inserts/updates the corresponding rows in `node_identity` and `peers`.
- `config`: Loads configuration from the environment (`GRAPHCHAN_API_PORT`, `GRAPHCHAN_PUBLIC_ADDRS`, `GRAPHCHAN_RELAY_URL`) and materialises the filesystem layout.
- `identity`: Manages GPG key generation (embedded `sequoia-openpgp` library), persists the Iroh secret key, and encodes/decodes friendcodes (`version:1`, `{peer_id, gpg_fingerprint, addresses[]}`).
- `database`: Embeds migrations and exposes rusqlite-based repository traits for threads, posts, peers, and files.
- `threading`: Implements thread CRUD and post insertion, including DAG edge management and view models consumed by the API.
- `files`: Streams uploads into the local `FsStore`, keeps path metadata in SQLite, mirrors bytes under `files/uploads/`, and persists blob tickets/checksums for redistribution.
- `peers`: Wraps friendcode registration, maintains peer metadata, and synthesises the local peer record on demand.
- `network`: Starts an Iroh router that hosts both the Graphchan gossip protocol and the `/iroh-bytes/4` blobs protocol, tracks live connections, serialises outbound gossip events, and helps dial friendcodes.
- `node`: Wraps bootstrap + network start-up (`GraphchanNode`) so other binaries (CLI, REST daemon, desktop bundle) can reuse the same initialisation pipeline without duplicating code.
- `api`: Defines the Axum router and handlers for health, thread/post/file operations, and peer management. Emits networking events after local mutations.
- `cli`: Hosts the interactive shell and the HTTP server entrypoint. The shell lets operators manage friendcodes, inspect threads, post messages, and transfer attachments (`upload`, `download`) without a REST client. Online/offline status is inferred from active Iroh connections.
- `graphchan_desktop` (workspace crate): Spawns a `GraphchanNode`, launches the REST server on a background Tokio runtime, sets `GRAPHCHAN_API_URL` to the embedded port, and then runs the egui UI so the bundled binary “just works” while still exposing the HTTP API to other clients.
- `utils`: Houses shared helpers such as time utilities.

## Identity & Provisioning Flow
1. `graphchan_backend serve` loads configuration via `GraphchanConfig::from_env`.
2. `bootstrap::initialize` ensures directories exist and connects to SQLite.
3. Migrations execute on first run, and schema drift (e.g., new columns) is handled idempotently.
4. GPG keys are generated internally using `sequoia-openpgp` if missing; the fingerprint is written to `keys/gpg/fingerprint.txt` and exported to `public.asc`/`private.asc`.
5. An Iroh secret key is generated (versioned JSON) if `keys/iroh.key` is absent.
6. A friendcode combining `{peer_id, gpg_fingerprint, advertised addresses}` is produced and stored in both `node_identity` and the local `peers` row.
7. The Iroh endpoint boots (respecting relay configuration) and the Axum HTTP server starts listening.

## Networking Strategy
- **Transport**: A single Iroh router per node that accepts both the custom `graphchan/0` gossip ALPN and `/iroh-bytes/4` for blob transfers. If `GRAPHCHAN_RELAY_URL` is set, the underlying endpoint binds with `RelayMode::Custom` so traffic is routed via the configured relay.
- **Gossip Protocol**: Uses `iroh-gossip` for P2P message broadcasting. All peers subscribe to the `graphchan-global` topic at startup. When peers connect via friendcode, additional topic subscriptions are created with the peer as a bootstrap node to establish gossip mesh connectivity. Messages are automatically deduplicated by iroh-gossip, and neighbor up/down events track mesh connectivity.
- **Events**: A Tokio `mpsc` channel buffers outbound `NetworkEvent`s. Each event is framed as `EventEnvelope {version, topic, payload}` (JSON) and broadcast to all connected peers via gossip. Payloads include:
  - `ThreadSnapshot(ThreadDetails)` - complete thread with posts and peer metadata
  - `PostUpdate(PostView)` - single post update
  - `FileAvailable(FileAnnouncement)` - announces a file with its `BlobTicket` for download
  - `ProfileUpdate(ProfileUpdate)` - peer profile/avatar updates
  - Legacy `FileRequest`/`FileChunk` retained for compatibility but no longer used
- **File Synchronization**: Full iroh-blobs integration with active blob downloads:
  1. **Upload**: Files are stored in the local `FsStore` (content-addressed by Blake3 hash), with metadata persisted to SQLite including the blob hash and a `BlobTicket` containing the uploader's endpoint address.
  2. **Announcement**: `FileAvailable` gossip message broadcasts the ticket to all peers.
  3. **Download**: When a peer receives a `FileAvailable` with a ticket:
     - Checks if blob exists locally (`blob_store.has(hash)`)
     - If missing, uses `blob_store.downloader(&endpoint).download(hash, peer_id)` to actively pull the blob from the peer specified in the ticket
     - Exports blob to `files/downloads/{file_id}` via `blob_store.export(hash, path)`
     - Updates SQLite with path, size, and checksum
  4. **Deferred Download**: If a `FileAvailable` arrives before its post exists (race condition), the file record is saved and download is deferred until `ThreadSnapshot` creates the post.
- **Peer Ingestion**: When receiving a `ThreadSnapshot`, peers from the snapshot are ingested first, then stub peer records are created for any missing post authors to satisfy foreign key constraints before posts are inserted.
- **Friendcodes**: The API decodes friendcodes into `EndpointAddr` values and instructs the network layer to dial the peer immediately, then subscribes to the global gossip topic with that peer as a bootstrap node to establish mesh connectivity. Addresses may be socket addresses or relay URLs.

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

## Request Lifecycle Examples

### Thread Creation with Attachments
1. Client `POST /threads` with multipart form data (title, body, optional files).
2. `ThreadService` validates input, writes thread and seed post via repositories.
3. For each attachment:
   - File bytes are imported into `FsStore` via `blob_store.add_bytes()`, obtaining a Blake3 hash
   - Metadata is persisted to SQLite: `{id, post_id, path, original_name, mime, blob_id, size_bytes, checksum, ticket}`
   - A `BlobTicket` is created with the local endpoint address and blob hash
   - `FileAvailable` gossip message is broadcast with the ticket
4. `ThreadSnapshot` is broadcast to all peers via gossip
5. Response returns the complete `ThreadDetails` with posts and file metadata

### File Download on Remote Peer
1. Peer receives `FileAvailable` gossip message with `BlobTicket`
2. `apply_file_announcement` saves file record to SQLite
3. If post exists, spawns `download_blob` task:
   - Checks if blob exists locally: `blob_store.has(hash)`
   - If missing, downloads from peer: `blob_store.downloader(&endpoint).download(hash, peer_id)`
   - Exports to downloads directory: `blob_store.export(hash, path)`
   - Updates SQLite with final path, size, checksum
4. If post doesn't exist yet (race condition), download is deferred
5. When `ThreadSnapshot` arrives and creates the post, deferred download is triggered

### Handling Race Conditions
The gossip protocol provides no ordering guarantees, so:
- **FileAvailable before ThreadSnapshot**: File record is saved, download deferred until post exists
- **Missing peer records**: Stub peers are created automatically with `trust_state: "unknown"` before inserting posts
- **Duplicate messages**: iroh-gossip handles deduplication automatically
- **Blob already exists**: Download is skipped, file is exported from local store directly

## Desktop Bundle & Portability
- The workspace now ships a `graphchan_desktop` binary that links both crates. It bootstraps a `GraphchanNode` via the shared `node` module, spawns the Axum/iroh services on a dedicated Tokio runtime, and sets `GRAPHCHAN_API_URL` to the bound localhost port before launching the egui UI.
- Because the backend still derives `GraphchanPaths` from `current_exe().parent()`, copying the desktop binary to a new directory continues to create isolated `data/`, `files/`, `blobs/`, and `keys/` trees—multiple instances on the same machine behave like separate peers.
- Operators who prefer headless deployments can continue running `cargo run --bin graphchan_backend -- serve` (REST) or `-- cli`; the desktop app simply layers the GUI on top of the exact same API surface, so third-party clients remain fully supported.
- When the GUI exits, the desktop launcher aborts the Axum server task and calls `NetworkHandle::shutdown()` to tear down the embedded router cleanly, ensuring no stray QUIC endpoints stay bound between sessions.

## Known Gaps / Future Work
1. **Blob download retry logic** – Currently blob downloads fail silently if the peer is unreachable; add exponential backoff and retry queue for failed downloads.
2. **Legacy FileRequest/FileChunk removal** – The old gossip-based file transfer code is retained for compatibility but no longer used; can be removed once all nodes are updated.
3. **Blob redemption tests** – Add automated multi-node integration tests that exercise ticket issuance, gossip propagation, and active blob downloads end to end.
4. **API polish** – Add pagination/search, richer error mapping, and authentication hooks.
5. **Observability** – Expand logging, metrics, and tracing to aid operators once nodes run long-lived.
6. **Peer discovery improvements** – Currently requires manual friendcode exchange; explore DHT or rendezvous for automatic peer discovery.

## Documentation & Tooling
- Architectural intent lives here; design rationales and roadmap details are in `Docs/Design.md` and `Docs/ImplementationPlan.md`.
- `AGENTS.md` captures workflow guidance for contributors.
- Run `cargo fmt`, `cargo check`, `cargo clippy --all-targets -- -D warnings`, and `cargo test --all` before committing. Use `cargo run --bin graphchan_backend` to launch a development node.
