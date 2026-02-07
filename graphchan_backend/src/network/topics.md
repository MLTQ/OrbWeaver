# topics.rs

## Purpose
Topic ID derivation functions for gossip channel addressing. Provides deterministic topic generation from names or secrets, enabling both public discovery and private communication.

## Components

### `derive_topic_id`
- **Does**: Derives topic ID from human-readable name
- **Algorithm**: `blake3("topic:{name}")`
- **Use case**: User-subscribed topics like "tech", "art", "gaming"

### `derive_global_topic`
- **Does**: Returns well-known global topic ID
- **Constant**: `GLOBAL_TOPIC_NAME = "graphchan-global-v1"`
- **Status**: DEPRECATED - use user-defined topics instead

### `derive_social_thread_topic`
- **Does**: Derives private topic for social threads
- **Algorithm**: `blake3("orbweaver-social-v1:{thread_id}:{secret}")`
- **Use case**: Threads visible only to peers who know the secret

### `derive_private_thread_topic`
- **Does**: Derives topic for encrypted private threads
- **Algorithm**: `blake3("orbweaver-private-v1:{thread_id}:{secret}")`
- **Use case**: Threads with additional content encryption

### `derive_thread_topic`
- **Does**: Convenience dispatcher by visibility type
- **Maps**: "global" → global, "private" → private, "social"/default → social

## Topic Types

| Type | Discovery | Content | Use Case |
|------|-----------|---------|----------|
| Global | Public (well-known) | Plaintext | Public announcements |
| Named | Public (known name) | Plaintext | Community topics |
| Social | Secret (shared secret) | Plaintext | Friends-only threads |
| Private | Secret (shared secret) | Encrypted | Confidential threads |

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `network.rs` | Topic derivation functions | Algorithm changes |
| `ingest.rs` | Same topic IDs across nodes | Hash changes |

## Security Properties

- **Deterministic**: Same inputs always produce same topic ID
- **Unguessable**: Social/private topics require 32-byte secret
- **Collision-resistant**: Blake3 hash provides 256-bit security
- **No metadata leak**: Topic ID reveals nothing about content

## Notes
- Topic secrets typically shared via friend codes or out-of-band
- Different prefixes ensure social ≠ private even with same secret
- Global topic deprecated due to spam/moderation concerns
- Named topics enable community building without central coordination
