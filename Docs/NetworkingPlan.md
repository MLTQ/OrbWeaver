# Networking Plan

## Goals
- Use Iroh 0.94.0 endpoints to propagate Graphchan events (threads, posts, file availability).
- Support multi-node operation with optional custom relay configuration and advertised public addresses.
- Provide a resilient gossip pipeline that tolerates intermittent connectivity and duplicates.
- Share file payloads via Iroh blobs while keeping REST uploads local-first.

## Event Model
- **Topics:**
  - `thread:{thread_id}` – thread snapshots and post increments.
  - `file:{file_id}` – attachment announcements.
  - `control` – reserved for future peer metadata.
- **Payloads:** JSON `EventEnvelope { version, topic, payload }` framed with a u32 length prefix.
  - `ThreadSnapshot` carries `ThreadDetails` (summary + posts) so receivers can upsert missing content.
  - `PostUpdate` ships a single `PostView` when only a reply changes.
  - `FileAvailable` includes `{id, post_id, original_name, mime, size_bytes, checksum?, blob_id?}`; blob info is still optional until promotion lands.
  - `FileRequest { file_id }` allows a peer missing a blob to request it directly from the announcer.
  - `FileChunk { file_id, data, eof }` transports the raw bytes back to the requester (currently bundled as a single chunk per file).
  - Add `node_signature` using the local GPG key for authenticity once signing helpers land.

## Transport Strategy
1. **Endpoint Lifecycle**
   - Bind endpoint with `GRAPHCHAN_ALPN` (already done).
   - Respect `GRAPHCHAN_RELAY_URL` by building a `RelayMap` when present (done).
   - Periodically refresh `EndpointAddr` and publish local friendcode updates when addresses change.

2. **Gossip Channels**
   - Use Iroh’s stream APIs (`Endpoint::open_bi`/`accept_bi`) to open long-lived connections per peer.
   - Implement a simple protocol on top of QUIC streams:
     - Frame = `<len:u32><payload>` where payload is `EventEnvelope` (topic + bytes).
     - Broadcast events fan out to every active connection; targeted messages (`FileRequest`, `FileChunk`, future control frames) resolve the remote peer ID and send over a single connection.
   - Alternatively, explore `iroh::net` pub/sub if exposed (to be researched in Docs/Iroh).

3. **Peer Discovery & Sessions**
   - Decode incoming friendcode → `EndpointAddr`. Add to connection manager.
   - Maintain a background task that dials known peers; on success, register a `PeerSession` and spawn reader/writer tasks.
   - Support reconnection with exponential backoff.

4. **Blob Distribution**
   - When uploading a file:
     1. Store locally and compute checksum.
     2. Stage file as an Iroh blob (map local path → blob hash via Iroh Blob API).
     3. Emit `FileEvent` with `blob_hash` so peers can fetch via Iroh.
   - On receipt of `FileEvent`, if blob missing, fetch via Iroh and persist under `files/downloads/`.

5. **Ordering & Idempotency**
   - Each `PostEvent` carries `post_id` so receivers can upsert (using existing repositories’ `upsert` logic to implement).
   - Maintain event revision numbers (timestamp or monotonic counters) to de-duplicate.

## Implementation Roadmap
1. **Message Layer** ✅ `network/events.rs` now serializes full thread/post/file payloads and broadcasts to every live connection.

2. **Session Manager** ✅ Accept + dial loops register connections, track lifecycle, and spawn reader tasks per QUIC connection.

3. **Inbound Processing** ✅ `network/ingest.rs` receives decoded envelopes and upserts data into SQLite (threads, posts, files) so REST reads reflect remote gossip.

4. **Blob Integration**
   - Current: single-chunk transfers ride over the gossip stream after a `FileRequest`; attachments are verified with Blake3 and stored under `files/downloads/`.
   - Next: research the native Iroh blob API (`iroh::blobs`) to stream large payloads, resume partial downloads, and decouple file sync from the control channel.

5. **Testing**
   - Add multi-node integration harness launching two servers with loopback friendcodes.
   - Validate thread + file replication end-to-end (after blob promotion lands update the ignored REST test).

## Open Questions
- Should friendcodes embed relay/public address updates each time? Might need signed metadata messages over gossip instead.
- Encryption: rely on Iroh’s transport security; do we also sign payloads with GPG? (Recommended for tamper detection.)
- Persistence of peer sessions: write to DB for crash recovery?

## References
- `Docs/Iroh Docs 0.94.0/src/iroh/endpoint.rs.html` for endpoint builder, relays, and connection APIs.
- `iroh-base::EndpointAddr` for managing addresses and relays.
- Future: check `iroh::net` module for higher-level pub/sub once stable.
