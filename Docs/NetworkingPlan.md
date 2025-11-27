# Networking Plan

## Goals
- Use Iroh 0.94.0 endpoints to propagate Graphchan events (threads, posts, file availability).
- Support multi-node operation with optional custom relay configuration and advertised public addresses.
- Provide a resilient gossip pipeline that tolerates intermittent connectivity and duplicates.
- Share file payloads via Iroh blobs while keeping REST uploads local-first.

## Event Model
- **Topics:**
  - `graphchan-global` – All messages are routed to this single global topic. Per-peer topics are created during friendcode connection (with peer as bootstrap) but all use the same global topic ID for message routing.
  - Topic-based filtering was simplified to use a single global channel to avoid message loss.
- **Payloads:** JSON `EventEnvelope { version, topic, payload }` serialized directly (no length prefix needed with iroh-gossip).
  - `ThreadSnapshot(ThreadDetails)` – Carries complete thread with posts and peer metadata so receivers can upsert missing content. Peers list includes all authors referenced in posts to satisfy foreign key constraints.
  - `PostUpdate(PostView)` – Ships a single post when only a reply changes.
  - `FileAvailable(FileAnnouncement)` – Includes `{id, post_id, thread_id, original_name, mime, size_bytes, checksum?, blob_id?, ticket: BlobTicket}`. The ticket contains the uploader's endpoint address and blob hash for active download.
  - `ProfileUpdate(ProfileUpdate)` – Peer profile/avatar updates with optional blob ticket for avatar files.
  - **DEPRECATED** `FileRequest { file_id }` and `FileChunk { file_id, data, eof }` – Legacy gossip-based file transfer retained for compatibility but no longer used. Replaced by iroh-blobs active downloads.
  - Future: Add `node_signature` using the local GPG key for authenticity once signing helpers land.

## Transport Strategy
1. **Endpoint Lifecycle** ✅
   - Bind endpoint with `GRAPHCHAN_ALPN = b"graphchan/0"` (done).
   - Respect `GRAPHCHAN_RELAY_URL` by building a `RelayMap` when present (done).
   - Future: Periodically refresh `EndpointAddr` and publish local friendcode updates when addresses change.

2. **Gossip Protocol** ✅
   - Uses `iroh-gossip` (`iroh_gossip::net::Gossip`) for P2P message broadcasting.
   - All peers subscribe to `graphchan-global` topic at startup via `gossip.subscribe(topic_id, vec![])`.
   - When connecting to a peer via friendcode:
     1. Endpoint connects: `endpoint.connect(addr, GRAPHCHAN_ALPN).await`
     2. Creates new gossip subscription with peer as bootstrap: `gossip.subscribe(global_topic_id, vec![peer_id])`
     3. Spawns receiver loop for the peer-specific subscription
   - Messages are JSON-serialized `EventEnvelope` and broadcast via `topic.broadcast(Bytes::from(json))`.
   - iroh-gossip handles:
     - Automatic mesh formation between neighbors
     - Message deduplication
     - Neighbor up/down events for connectivity tracking
     - Reliable delivery within the mesh

3. **Peer Discovery & Sessions** ✅
   - Decode incoming friendcode → `EndpointAddr` (done).
   - `NetworkHandle::connect_friendcode` initiates endpoint connection and gossip subscription (done).
   - Multiple parallel gossip receiver loops (one from startup, one per friendcode connection) all forward to a single inbound channel.
   - Future: Support reconnection with exponential backoff for failed connections.

4. **Blob Distribution** ✅
   - **Upload path:**
     1. File bytes imported to `FsStore`: `blob_store.add_bytes(bytes).temp_tag().await`
     2. Obtain Blake3 hash from tag: `hash_and_format().hash`
     3. Create `BlobTicket` with local endpoint address: `BlobTicket::new(endpoint_addr, hash, BlobFormat::Raw)`
     4. Persist metadata to SQLite including ticket string
     5. Broadcast `FileAvailable` with ticket via gossip
   - **Download path:**
     1. Receive `FileAvailable` with `BlobTicket`
     2. Save file record to SQLite (even if post doesn't exist yet)
     3. If post exists, spawn download task:
        - Check local store: `blob_store.has(hash)`
        - If missing, download from peer: `blob_store.downloader(&endpoint).download(hash, peer_id)`
        - Export to downloads: `blob_store.export(hash, path)`
        - Update SQLite with final path/size/checksum
     4. If post missing, defer download until `ThreadSnapshot` creates it
   - Files of any size supported; no gossip message size limits.

5. **Ordering & Idempotency** ✅
   - Events carry stable IDs (`thread_id`, `post_id`, `file_id`) enabling upsert operations.
   - Repository `upsert` methods handle duplicates idempotently (done).
   - iroh-gossip provides automatic message deduplication.
   - Race conditions handled:
     - `FileAvailable` before `ThreadSnapshot`: File record saved, download deferred
     - Missing peer records: Stub peers created automatically
     - Duplicate `ThreadSnapshot`: upsert is idempotent

## Implementation Roadmap
1. **Message Layer** ✅ `network/events.rs` serializes full thread/post/file payloads and broadcasts via iroh-gossip to all connected peers on `graphchan-global` topic.

2. **Gossip Integration** ✅ Migrated from custom QUIC streams to `iroh-gossip` for reliable P2P broadcasting with automatic mesh formation, deduplication, and neighbor tracking.

3. **Inbound Processing** ✅ `network/ingest.rs` receives decoded envelopes and upserts data into SQLite (threads, posts, files, peers) with proper handling of foreign key constraints and race conditions.

4. **Blob Integration** ✅
   - ~~Legacy: single-chunk transfers over gossip after `FileRequest` (deprecated, retained for compatibility)~~
   - **Current**: Full iroh-blobs integration with active downloads:
     - Files stored in content-addressed `FsStore` with Blake3 hashing
     - `BlobTicket` creation and gossip broadcast
     - Active blob download: `blob_store.downloader(&endpoint).download(hash, peer_id)`
     - Export to downloads directory via `blob_store.export(hash, path)`
     - Deferred downloads for race conditions
   - Supports files of any size without message size limits
   - Blobs are deduplicated by hash automatically

5. **Testing** ⚠️
   - Manual two-node testing confirms end-to-end replication with blob downloads working
   - TODO: Add automated integration harness launching two servers with loopback friendcodes
   - TODO: Validate thread + file replication in CI

## Open Questions
- Should friendcodes embed relay/public address updates each time? Might need signed metadata messages over gossip instead.
- Encryption: Currently relying on Iroh's transport security (QUIC + TLS). Future: sign payloads with GPG for tamper detection and non-repudiation.
- Persistence of peer sessions: Currently peer metadata is persisted but connection state is ephemeral. Consider persisting connection attempts for crash recovery?
- Blob download retry: Should failed downloads be queued for retry with exponential backoff, or rely on user-initiated refresh?
- Legacy code removal: When can we remove deprecated `FileRequest`/`FileChunk` handlers?

## References
- `Docs/Iroh Docs 0.94.0/src/iroh/endpoint.rs.html` for endpoint builder, relays, and connection APIs.
- `iroh-base::EndpointAddr` for managing addresses and relays.
- `iroh-gossip` crate documentation: https://docs.rs/iroh-gossip/latest/iroh_gossip/
- `iroh-blobs` crate documentation: https://docs.rs/iroh-blobs/latest/iroh_blobs/
- Iroh quickstart guide: https://www.iroh.computer/docs/quickstart
- BlobTicket API: https://docs.rs/iroh-blobs/latest/iroh_blobs/ticket/struct.BlobTicket.html
