# network/ingest/mod.rs

## Purpose
Inbound gossip application pipeline for the backend. It validates, deduplicates, applies, and rebroadcasts network events, and it records per-peer connection metadata needed by moderation features.

## Components

### `run_ingest_loop`
- **Does**: Receives inbound gossip frames, applies them, and triggers follow-up work like thread resyncs.
- **Interacts with**: `events.rs` for payload types, `resync.rs` for snapshot recovery, `EventPublisher` in `events.rs`.

### `capture_peer_ip`
- **Does**: Maps an inbound iroh endpoint id to a canonical peer id and records the best available direct IP address for blocking and moderation.
- **Interacts with**: `Database` peer repositories, `Endpoint` in `../network.rs`.
- **Rationale**: Keeps IP-based moderation tied to live transport state rather than untrusted payload fields.
- **Path selection**: Uses `endpoint.remote_info(...)` and keeps only `TransportAddrUsage::Active` `TransportAddr::Ip(...)` entries, so relay-only paths never create synthetic peer IP records.

### `handle_message`
- **Does**: Dispatches each `EventPayload` variant to the appropriate apply/download path and decides whether to rebroadcast or trigger resync.
- **Interacts with**: `files.rs`, `profile.rs`, `reactions.rs`, `resync.rs`, repository layer.

### `mark_seen`
- **Does**: Maintains the bounded LRU dedup cache for gossip rebroadcast suppression.
- **Interacts with**: `run_ingest_loop`.

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `network.rs` | `run_ingest_loop` consumes inbound gossip and never panics on malformed payloads | Signature, task lifetime, panic behavior |
| moderation / IP blocking flow | `capture_peer_ip` stores direct peer IPs only when transport state exposes them | Removing IP capture or changing direct-vs-relay rules |
| rebroadcast logic | duplicate message ids are suppressed within a bounded cache window | Dedup key format or cache semantics |

## Notes
- Direct and mixed connections can expose a usable remote IP; relay-only connections generally cannot.
- Resync work is intentionally detached into background tasks so ingest stays responsive under hash mismatch recovery.
