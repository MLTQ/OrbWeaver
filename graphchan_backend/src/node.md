# node.rs

## Purpose
High-level node lifecycle management providing a convenient wrapper around bootstrap, networking, and service initialization. Entry point for CLI, REST server, and embedded UI modes.

## Components

### `GraphchanNode`
- **Does**: Owns all backend resources, provides unified startup
- **Fields**: config, bootstrap (resources), blob_store, network
- **Pattern**: Start once, snapshot for consumers

### `GraphchanNode::start`
- **Does**: Full node initialization sequence
- **Flow**:
  1. `bootstrap::initialize(config)` - Create dirs, init DB, load/generate keys
  2. `FsStore::load(blobs_dir)` - Load Iroh blob store
  3. `NetworkHandle::start(...)` - Initialize P2P networking
- **Returns**: Ready-to-use `GraphchanNode`

### `snapshot`
- **Does**: Returns cloneable handles for service consumers
- **Returns**: `NodeSnapshot` with all service handles
- **Use case**: Share access without transferring ownership

### `run_http_server`
- **Does**: Starts REST API server using node resources
- **Blocks**: Until shutdown signal
- **Delegates to**: `api::serve_http`

### Accessor Methods
- `identity()` - Local GPG fingerprint, Iroh peer ID, friend code
- `database()` - Clone of database handle
- `network()` - Clone of network handle
- `blobs()` - Clone of blob store

### `NodeSnapshot`
- **Does**: Cloneable bundle of all service handles
- **Fields**: config, identity, database, network, blobs
- **Use case**: Pass to handlers without owning node

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `GraphchanNode::start(config)` | Initialization changes |
| `graphchan_desktop` | `snapshot()` for API server | Snapshot contents |
| `cli.rs` | Service accessors | Method removal |

## Initialization Sequence

```
GraphchanNode::start(config)
    │
    ├── bootstrap::initialize()
    │   ├── Create directories (data, keys, files, logs)
    │   ├── Initialize SQLite database
    │   ├── Load or generate GPG keypair
    │   └── Load or generate Iroh secret key
    │
    ├── FsStore::load()
    │   └── Initialize content-addressed blob storage
    │
    └── NetworkHandle::start()
        ├── Create Iroh Endpoint
        ├── Setup Gossip + Blobs protocols
        └── Spawn event/ingest worker tasks
```

## Notes
- Node is designed for single-instance operation
- All resources are Clone-able for multi-threaded access
- Shutdown is graceful (workers clean up on drop)
- Configuration via `GraphchanConfig` (env vars or explicit)
