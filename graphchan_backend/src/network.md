# network.rs

## Purpose
P2P networking layer built on Iroh, providing gossip-based message propagation and content-addressed file transfer. Manages endpoint setup, topic subscriptions, and coordinates between gossip and blob protocols.

## Components

### `NetworkHandle`
- **Does**: Main interface to the networking stack
- **Fields**: endpoint, gossip, publisher channels, topic maps, blobs, database
- **Pattern**: Clone-able for concurrent access across handlers

### `NetworkHandle::start`
- **Does**: Initializes the full networking stack
- **Flow**:
  1. Load Iroh secret key
  2. Configure relay mode (custom or default)
  3. Add discovery (mDNS, optional DHT)
  4. Build Endpoint with Router (multiplexes ALPN protocols)
  5. Start Gossip protocol
  6. Spawn event and ingest worker tasks

### Protocol Integration
- **Gossip ALPN**: Message propagation via iroh-gossip
- **Blobs ALPN**: Content-addressed file transfer via iroh-blobs
- **Router**: Multiplexes both protocols on single endpoint

### Topic Management

#### `subscribe_to_topic`
- **Does**: Joins a gossip topic, spawns listener task
- **Interacts with**: `topics` RwLock map, gossip API

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

## Submodules

- **events** - Event types and gossip message handling
- **ingest** - Inbound message processing pipeline
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
