# network.rs

## Purpose
P2P networking layer built on Iroh, providing gossip-based message propagation and content-addressed file transfer. Manages endpoint setup, topic subscriptions, and coordinates between gossip and blob protocols.

## Components

### `NetworkHandle`
- **Does**: Main interface to the networking stack
- **Fields**: endpoint, gossip, publisher channels, topic maps, blobs, database, static_provider
- **Pattern**: Clone-able for concurrent access across handlers

### `NetworkHandle::start`
- **Does**: Initializes the full networking stack
- **Flow**:
  1. Load Iroh secret key
  2. Configure relay mode (custom or default)
  3. Add discovery (StaticProvider, mDNS, optional DHT)
  4. Build Endpoint with Router (multiplexes ALPN protocols)
  5. Start Gossip protocol
  6. Spawn event and ingest worker tasks

### Protocol Integration
- **Gossip ALPN**: Message propagation via iroh-gossip
- **Blobs ALPN**: Content-addressed file transfer via iroh-blobs
- **Router**: Multiplexes both protocols on single endpoint

### Topic Management

#### `gather_friend_bootstrap_peers`
- **Does**: Collects all known friends' iroh PublicKeys from the database
- **Use case**: Bootstrap gossip subscriptions with friends for fast, reliable discovery
- **Pattern**: Called by subscribe_to_topic, subscribe_to_global, subscribe_to_peer

#### `subscribe_to_topic`
- **Does**: Joins a gossip topic, spawns listener task
- **Discovery**: Three-layer approach:
  1. **Friend bootstrapping (PRIMARY)**: All known friends' iroh IDs passed as bootstrap peers. If friend is online and on same topic, iroh-gossip connects directly via Pkarr address resolution. Fast and reliable.
  2. **DHT auto-discovery via DTT (SECONDARY)**: `distributed-topic-tracker` publishes/discovers peers via BEP44 mutable records on BitTorrent mainline DHT. Slower, for discovering strangers. Limited because records only contain node_id (no relay/direct addrs).
  3. **Schelling point discovery (TERTIARY)**: Custom BEP44 records containing full EndpointAddr (node_id + relay URL + direct addrs). All peers on the same topic derive identical BEP44 signing keys from topic name + minute window. Records are encrypted with ChaCha20Poly1305 so only peers who know the topic name can read them. Discovered addresses injected into StaticProvider for iroh resolution.
- **Interacts with**: `topics` RwLock map, gossip API, PeerService, distributed-topic-tracker, schelling module, StaticProvider

#### `broadcast_to_topic`
- **Does**: Sends message to all peers on a topic
- **Interacts with**: Gossip topic sender

### DHT Integration

#### `DhtTopicSender`
- **Does**: Wrapper for broadcasting to DHT-discovered peers
- **Interacts with**: `distributed_topic_tracker` crate

#### `DhtStatus`
- **Does**: Tracks DHT connectivity (Checking/Connected/Unreachable)
- **Use case**: UI feedback on network status

#### DHT Record Identity (CRITICAL)
- **Rule**: DHT records MUST use the iroh endpoint secret key for signing
- **Why**: `record.node_id()` is extracted by peers during bootstrap and used as an iroh EndpointId for `join_peers()`. If node_id doesn't match a real iroh endpoint, peers can never connect.
- **Field**: `iroh_secret_bytes` stores the endpoint key for use in `subscribe_to_topic()`
- **BEP44**: Records published via shared topic-derived signing key (not the per-peer key). The per-peer key goes inside the encrypted record as `node_id`.

### StaticProvider
- **Does**: Injects out-of-band peer addresses into iroh's discovery system
- **Created in**: `NetworkHandle::start()`, added to endpoint builder discovery chain
- **Used by**: Schelling discovery loop to inject discovered peer addresses
- **Pattern**: `add_endpoint_info(EndpointAddr)` merges relay URLs and direct addrs for a peer

### Publishing Methods

#### `publish_direct_message`
- **Does**: Broadcasts encrypted DM event over gossip
- **Routing**: Goes to `peer-{to_peer_id}` topic

#### `publish_block_action`
- **Does**: Broadcasts block/unblock action over gossip
- **Routing**: Goes to `peer-{blocker_peer_id}` topic for blocklist subscribers

## Re-exports

- `BlockActionEvent`, `DirectMessageEvent`, `FileAnnouncement`, `ProfileUpdate`, `ReactionUpdate` — from `events` submodule

## Submodules

- **events** - Event types and gossip message handling
- **ingest** - Inbound message processing pipeline
- **schelling** - Schelling point BEP44 discovery for topic-based peer finding
- **topics** - Topic ID derivation functions

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api.rs` | `NetworkHandle` methods for broadcasting | Method changes |
| `node.rs` | `NetworkHandle::start()` signature | Initialization changes |

## Constants

- `GRAPHCHAN_ALPN = b"graphchan/0"` - Custom protocol identifier
- `GOSSIP_BUFFER = 128` - Channel buffer size

## Notes
- Uses n0's default public relays unless custom relay configured
- mDNS enables local network discovery
- DHT discovery optional (can be disabled via config)
- Topic subscriptions persist across node restarts
- Each topic has dedicated receiver task for isolation
