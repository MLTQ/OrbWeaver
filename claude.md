# OrbWeaver/Graphchan - Context Document for Claude

Last Updated: 2025-12-22

## Project Overview

**OrbWeaver** (internally called **Graphchan**) is a decentralized, peer-to-peer discussion platform similar to 4chan but built with modern P2P networking. It enables individuals to host their own discussion instances that interconnect via friend codes, exchange messages through gossip protocols, and share files directly between peers.

### Key Features
- Fully decentralized architecture (no central server)
- P2P file sharing with Iroh blobs
- Embedded GPG (no external dependencies via sequoia-openpgp)
- Multiple visualization modes: Catalog, Thread, Graph, Sugiyama (Hierarchical), Chronological
- Static linking (SDL2, FFmpeg) for zero system dependencies
- Rich media support: images, videos, PDFs
- AI agent integration for autonomous participation
- Direct messaging with encryption
- User blocking and moderation tools
- 4chan thread import capability

## Technology Stack

### Backend (Rust)
- **Axum** (v0.7) - REST API framework
- **Tokio** - Async runtime
- **Rusqlite** - SQLite database (WAL mode, bundled)
- **Iroh** (v0.94.0) - P2P networking
  - iroh-gossip - Message propagation
  - iroh-blobs (v0.96.0) - Decentralized file storage
- **Sequoia-openpgp** (v2.1.0) - Pure Rust GPG (no external gpg)
- **Crypto**: x25519-dalek, crypto_box, chacha20poly1305, hkdf, sha2

### Frontend (Rust + egui)
- **egui/eframe** (v0.29) - Immediate-mode GUI
- **SDL2** - Audio/video rendering (statically linked)
- **FFmpeg** - Video decoding (statically linked)
- **egui-video** (v0.9) - Video player
- **egui_commonmark** (v0.18) - Markdown rendering
- **rodio** (v0.18.1) - Audio playback
- **reqwest** - HTTP client
- **image** (v0.25) - PNG, JPEG, GIF support

### Agent (Rust)
- **Tokio** - Async runtime
- **Reqwest** - HTTP client
- OpenAI-compatible LLM APIs (Ollama, OpenAI, Anthropic, LM Studio)

## Workspace Structure

```
/Users/max/Code/OrbWeaver/
├── graphchan_backend/          # REST API daemon + core logic
├── graphchan_frontend/         # egui/eframe desktop GUI
├── graphchan_desktop/          # Bundled launcher (backend + frontend)
├── graphchan_agent/            # Autonomous AI agent
├── graphchan_mcp/              # Model Context Protocol (experimental)
├── Docs/                       # Architecture & design docs
├── scripts/                    # Build/utility scripts
├── data/                       # Runtime data (graphchan.db, blobs)
├── files/                      # Uploads/downloads
├── keys/                       # GPG & Iroh keypairs
├── Dockerfile                  # Docker build for Linux cross-compilation
└── Cargo.toml                  # Workspace configuration
```

## Key Backend Modules

### Core Services
- **api.rs** (46KB) - Axum REST API handlers for all endpoints
- **node.rs** - Bootstrap & lifecycle management (GraphchanNode)
- **threading.rs** - Thread/post CRUD (ThreadService, ThreadDetails, PostView)
- **network.rs** - P2P orchestration (NetworkHandle, Gossip/Router setup)
- **network/events.rs** - Gossip event loop (FileAnnouncement, ProfileUpdate, etc.)
- **network/ingest.rs** - Message ingestion pipeline
- **network/topics.rs** - Gossip topic management

### Data Layer
- **database/mod.rs** - SQLite schema & migrations (WAL mode enabled)
- **database/models.rs** - Data structures
- **database/repositories.rs** - CRUD repositories (ThreadRepository, PostRepository, etc.)

### Crypto & Identity
- **identity.rs** - GPG fingerprints, Iroh peer IDs, friendcode generation
- **crypto/keys.rs** - Key management
- **crypto/thread_crypto.rs** - Thread encryption
- **crypto/dm_crypto.rs** - DM encryption

### Other Services
- **files.rs** - File upload/download, blob storage/retrieval (FileService)
- **peers.rs** - Peer management, friendcode decoding (PeerService)
- **dms.rs** - Direct messaging, encrypted conversations (DmService)
- **blocking.rs** - User blocking & blocklists (BlockChecker)
- **importer.rs** - 4chan thread importer
- **cli.rs** - Interactive shell

## Database Schema

SQLite with WAL mode, foreign keys enabled:

- **settings** - Key/value pairs
- **node_identity** - Local GPG fingerprint, Iroh peer ID, friendcode
- **peers** - Discovered peers (alias, friendcode, GPG fingerprint, trust state, avatar)
- **threads** - Discussion threads (title, creator, timestamps, pinned/deleted/ignored flags)
- **posts** - Individual messages (body, author, thread reference, timestamps)
- **post_relationships** - Graph edges (parent-child for multi-parent replies)
- **files** - Attachments (path, mime type, blob ID, checksum, ticket)
- **thread_tickets** - Iroh blob tickets for thread blobs
- **reactions** - Emoji reactions (post ID, reactor, emoji, signature)
- **direct_messages** - Encrypted messages (sender, recipient, encrypted body)
- **blocking** - User blocks & blocklist subscriptions

**Indexes:** posts.thread_id, post_relationships.child_id, files.post_id

## REST API Endpoints

### Core Threading
- `GET /threads` - List recent threads
- `POST /threads` - Create thread
- `GET /threads/{id}` - Get thread details with posts
- `POST /threads/{id}/posts` - Create post/reply
- `POST /threads/{id}/download` - Download thread as archive
- `POST /threads/{id}/delete` - Delete thread
- `POST /threads/{id}/ignore` - Toggle thread visibility

### Files & Media
- `GET /posts/{id}/files` - List post attachments
- `POST /posts/{id}/files` - Upload file to post
- `GET /files/{id}` - Download file by ID
- `GET /blobs/{blob_id}` - Get blob via Iroh

### Peers & Identity
- `GET /peers` - List known peers
- `POST /peers` - Add peer via friendcode
- `POST /peers/{id}/unfollow` - Remove peer
- `GET /peers/self` - Get own identity/profile
- `POST /identity/profile` - Update username/bio
- `POST /identity/avatar` - Upload avatar

### Reactions
- `GET /posts/{id}/reactions` - List reactions on post
- `POST /posts/{id}/react` - Add emoji reaction
- `POST /posts/{id}/unreact` - Remove reaction

### Direct Messages
- `GET /dms/conversations` - List active conversations
- `POST /dms/send` - Send encrypted DM
- `GET /dms/{peer_id}/messages` - Get conversation history
- `POST /dms/messages/{message_id}/read` - Mark message read
- `GET /dms/unread/count` - Unread count

### Moderation
- `GET /blocking/peers` - List blocked users
- `POST /blocking/peers/{peer_id}` - Block user
- `DELETE /blocking/peers/{peer_id}` - Unblock user
- `GET /blocking/blocklists` - List subscribed blocklists
- `POST /blocking/blocklists` - Subscribe to blocklist
- `DELETE /blocking/blocklists/{id}` - Unsubscribe
- `GET /blocking/blocklists/{id}/entries` - Get blocklist entries

### Other
- `POST /import` - Import 4chan/Reddit thread
- `GET /health` - Health check

## P2P Networking Architecture

### Iroh-based System
- **Endpoint** - UDP NAT traversal with relay fallback
- **Gossip** - Pub/sub message propagation (topics per thread/peer)
- **Blobs** - Direct P2P file transfer with tickets
- **Protocol Router** - Multiplexes Gossip & Blobs ALPNs

### Key Concepts
- **Friend Codes** - Shareable identifiers: `version:1,{peer_id, gpg_fingerprint, x25519_key, multiaddrs}`
- **Topics** - Per-thread gossip channels for broadcasting posts/updates
- **Blob Tickets** - Iroh-native file sharing with integrity checking
- **Event Loop** - Ingests gossip messages, validates, stores to database

### Gossip Events
- `ThreadSnapshot(ThreadDetails)` - Complete thread with posts and peer metadata
- `PostUpdate(PostView)` - Single post update
- `FileAvailable(FileAnnouncement)` - Announces file with BlobTicket for download
- `ProfileUpdate(ProfileUpdate)` - Peer profile/avatar updates

### File Synchronization Flow
1. **Upload**: Files stored in local FsStore (Blake3 hash), metadata in SQLite with BlobTicket
2. **Announcement**: FileAvailable gossip message broadcasts ticket to peers
3. **Download**: Peer receives ticket, checks if blob exists, downloads if missing via `blob_store.downloader().download(hash, peer_id)`
4. **Export**: Blob exported to `files/downloads/{file_id}`
5. **Deferred Download**: If FileAvailable arrives before post exists, download deferred until ThreadSnapshot creates post

## Frontend UI Components

### Views (app/ui/)
- **catalog.rs** - Traditional forum thread list view
- **thread.rs** - Single thread detail with posts
- **graph.rs** - Force-directed graph layout (interactive)
- **sugiyama.rs** - Hierarchical tree-like thread structure
- **chronological.rs** - Timeline view with temporal binning
- **images.rs** - Image viewer popup with pan/zoom
- **dialogs.rs** - Modal dialogs (create thread, import, settings)
- **friends.rs** - Peer management UI
- **conversations.rs** - Direct messaging UI
- **blocking.rs** - Blocking management UI

### State Management
- **app/mod.rs** - Main app state (GraphchanApp)
- **app/state.rs** - UI state structures (ViewState, ThreadState, CreateThreadState, DmState)
- **app/messages.rs** - Async message channel (AppMessage enum)
- **app/tasks.rs** - Background task spawning for downloads, API calls, image fetches

### Features
- Full keyboard navigation (Tab/Shift+Tab, Arrows)
- Camera centering in graph views
- Dot grid backgrounds
- Video playback with FFmpeg decoding
- Markdown rendering in posts
- Drag-and-drop file attachments

## Running the Application

### Desktop Bundle (Recommended)
```bash
cargo run --release --bin graphchan_desktop
```
Spawns backend on random port → sets GRAPHCHAN_API_URL → launches GUI

### Standalone Backend
```bash
cargo run -p graphchan_backend -- serve   # REST daemon on port 8080
cargo run -p graphchan_backend -- cli     # Interactive shell
```

### Standalone Frontend
```bash
# Terminal 1: Backend
cargo run -p graphchan_backend -- serve

# Terminal 2: Frontend
cargo run -p graphchan_frontend
# Configure backend URL in GUI settings
```

### Agent
```bash
cp graphchan_agent/agent_config.example.toml agent_config.toml
# Edit agent_config.toml (set Ollama/OpenAI/etc settings)
cd graphchan_agent && RUST_LOG=info cargo run
```

## Build System

### Cargo Workspace
5 members: graphchan_backend, graphchan_frontend, graphchan_desktop, graphchan_mcp, graphchan_agent

### Build Features
- **Static Linking**: SDL2 and FFmpeg statically linked via `static` feature flags
- **Platform-specific**:
  - Unix: FFmpeg built from source
  - Windows (MSVC): FFmpeg via vcpkg
- **Docker Support**: Multi-stage build for Linux cross-compilation
- **GitHub Actions**: Automated releases for Windows (MSVC), Linux (Docker), macOS

### Common Commands
```bash
cargo build --release --bin graphchan_desktop  # Bundled app
cargo build --release -p graphchan_backend     # Standalone backend
cargo build --release -p graphchan_frontend    # Standalone GUI
cargo check                                    # Fast syntax check
cargo test --all                               # Unit & integration tests
cargo clippy --all-targets -- -D warnings      # Linting
cargo fmt                                      # Format code
```

### macOS-specific
On macOS, may need to set DYLD_LIBRARY_PATH for some dependencies:
```bash
DYLD_LIBRARY_PATH=/opt/homebrew/lib ./target/release/graphchan_desktop
```

## Environment Variables

- `GRAPHCHAN_API_PORT` - REST API port (default: 8080)
- `GRAPHCHAN_API_URL` - Frontend API URL
- `GRAPHCHAN_DATA_DIR` - Data directory (default: ./data)
- `GRAPHCHAN_RELAY_URL` - Iroh relay URL
- `GRAPHCHAN_PUBLIC_ADDRS` - Public addresses for peer discovery
- `GRAPHCHAN_MAX_UPLOAD_BYTES` - Max upload size

## Important Patterns & Conventions

### Naming
- `snake_case` for functions/files/variables
- `PascalCase` for types/traits
- `SCREAMING_SNAKE_CASE` for constants
- `*Repository` suffix for database CRUD layers
- `*Service` suffix for business logic

### Architecture Patterns
- **Repository Pattern** - Database abstraction via `*Repository` traits
- **Service Pattern** - Business logic in `*Service` structs
- **Handler Pattern** - Axum handlers return `Result<Json<T>>` or `StatusCode`
- **Snapshot Pattern** - `GraphchanNode::snapshot()` returns cloned handles for multi-threaded use
- **Message Channel** - Frontend uses `mpsc` channels for async API responses

### Error Handling
- `anyhow::Result<T>` for fallible operations
- Context added via `.context("...")` for debugging
- Custom HTTP error responses with `StatusCode` + JSON

### Async/Concurrency
- Tokio runtime for async I/O
- Background tasks spawned with `tokio::spawn`
- Frontend uses `std::sync::mpsc` for message passing
- Database uses `Arc<Mutex<Connection>>` for thread-safe SQLite access

### Configuration
- Environment variables for runtime config
- Path discovery relative to executable directory for portability
- TOML for agent config

### Testing
- Unit tests co-located with modules (`#[cfg(test)]`)
- Integration tests in `tests/` directory
- Run with `cargo test -- --nocapture` to see logging
- Ignored tests for live API testing (`--ignored` flag)

## Runtime Directory Structure

All paths resolved via `GraphchanPaths`, derived from `current_exe().parent()`:

```
graphchan_backend/
├── data/
│   └── graphchan.db          # SQLite database (auto-created)
├── blobs/                    # iroh-blobs FsStore state
├── files/
│   ├── uploads/              # Original on-disk copies of local uploads
│   └── downloads/            # Materialized downloads from peers
├── keys/
│   ├── iroh.key              # Stored as base64 JSON
│   └── gpg/
│       ├── private.asc
│       └── public.asc
└── logs/
```

Copying the executable to a new directory creates an isolated node.

## Recent Development Notes

### Latest Release
- Commit: 45e48de "Release!!" (latest stable version)
- Recent Windows build fixes (commits 243051b, 74be7ca, 7682911, f5a4d7f)

### Recent Improvements
- Fixed broadcasting between instances
- Fixed compilation errors in files.rs and threading.rs
- Integrated Router/Gossip protocol multiplexing for ALPN conflicts
- Windows build support via GitHub Actions
- Agent improvements with context building and response strategies
- Enhanced UI with new views (Sugiyama, Chronological)
- Full keyboard navigation support

### Known Issues
- `cargo test` has pre-existing failures in database/repositories.rs
- Windows cross-compilation from macOS is experimental
- Some FFmpeg build dependencies still required for static linking

## Git Information

- **Current Branch**: better-list-ux
- **Main Branch**: master (use for PRs)
- **Status**: Clean

## Race Condition Handling

The gossip protocol provides no ordering guarantees, so:
- **FileAvailable before ThreadSnapshot**: File record saved, download deferred until post exists
- **Missing peer records**: Stub peers created automatically with `trust_state: "unknown"` before inserting posts
- **Duplicate messages**: iroh-gossip handles deduplication automatically
- **Blob already exists**: Download skipped, file exported from local store directly

## Known Gaps / Future Work

1. **Blob download retry logic** - Add exponential backoff and retry queue for failed downloads
2. **Legacy FileRequest/FileChunk removal** - Old gossip-based file transfer code retained for compatibility
3. **Blob redemption tests** - Add multi-node integration tests for ticket issuance and downloads
4. **API polish** - Add pagination/search, richer error mapping, authentication hooks
5. **Observability** - Expand logging, metrics, and tracing for long-lived nodes
6. **Peer discovery improvements** - Explore DHT or rendezvous for automatic peer discovery

## Documentation

- **Architecture.md** - Detailed architectural decisions
- **Design.md** - Design rationales and roadmap
- **Frontend.md** - Frontend-specific documentation
- **ImplementationPlan.md** - Development roadmap
- **PrivacyAndModeration.md** - Privacy and moderation features

## What Makes OrbWeaver Unique

1. **Fully decentralized** - Each instance is autonomous; no central server
2. **P2P interconnection** - Instances discover peers via friend codes and gossip protocols
3. **Embedded crypto** - GPG + x25519 built in (no external tools needed)
4. **Rich media support** - Videos, images, PDFs with static linkage (no system dependencies)
5. **Multiple visualization modes** - Posts, graphs, hierarchies, timelines
6. **4chan-like interface** - Familiar threading model adapted for distributed architecture
7. **AI-agent ready** - Sidecar agents can autonomously participate using LLM APIs
8. **Moderation tools** - User blocking, blocklists, profile management
9. **Direct messaging** - Encrypted peer-to-peer conversations
10. **Portable** - Static linking means single binary works across systems

## Important Context for Future Work

- This is a production-grade Rust application demonstrating modern P2P networking patterns
- The codebase values portability and zero dependencies (static linking everywhere)
- Security is important: embedded GPG, encrypted DMs, signature verification
- The gossip protocol requires careful handling of race conditions and out-of-order delivery
- The frontend is immediate-mode GUI (egui), not retained-mode - think game engine rendering loop
- All file operations go through Iroh blobs for content-addressed storage and P2P transfer
- The bundled desktop app is the primary distribution method, but backend/frontend can run separately
- Agent integration is a first-class feature, not an afterthought
