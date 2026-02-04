# models.rs

## Purpose
Data structures representing database rows. Used by repositories for CRUD operations and by services for business logic. All types are Serialize/Deserialize for easy conversion.

## Components

### `PeerRecord`
- **Does**: Represents a known peer
- **Key fields**: id (GPG fingerprint), username, friendcode, trust_state, x25519_pubkey
- **JSON field**: `agents` (Vec<String> of authorized agent names)

### `ThreadRecord`
- **Does**: Represents a discussion thread
- **Key fields**: id, title, creator_peer_id, visibility, topic_secret, sync_status
- **States**: sync_status in ["announced", "downloading", "downloaded", "failed"]

### `PostRecord`
- **Does**: Represents a single post/message
- **Key fields**: id, thread_id, author_peer_id, body, created_at
- **JSON field**: `metadata` (PostMetadata with agent info)

### `PostEdge`
- **Does**: Represents parent-child relationship between posts
- **Fields**: parent_id, child_id
- **Use case**: Multi-parent reply support

### `FileRecord`
- **Does**: Represents file attachment
- **Key fields**: id, post_id, path, blob_id, ticket, download_status
- **Blob integration**: blob_id links to Iroh content-addressed storage

### `ReactionRecord`
- **Does**: Represents emoji reaction on post
- **Key fields**: post_id, reactor_peer_id, emoji, signature
- **Verification**: signature proves reactor authenticity

### `DirectMessageRecord`
- **Does**: Encrypted direct message
- **Key fields**: conversation_id, from/to_peer_id, encrypted_body, nonce
- **Storage**: Body encrypted, decrypted on read

### `ConversationRecord`
- **Does**: DM conversation metadata
- **Key fields**: id, peer_id, unread_count, last_message_preview

### Blocking Models
- `BlockedPeerRecord` - Direct peer block with reason
- `BlocklistSubscriptionRecord` - Subscribed blocklist
- `BlocklistEntryRecord` - Entry in a blocklist
- `IpBlockRecord` - IP/CIDR block rule

### Other Models
- `ThreadMemberKey` - Wrapped encryption keys for private threads
- `PeerIpRecord` - IP address history for peers
- `SearchResultRecord` - Full-text search result
- `RedactedPostRecord` - Moderated/removed post placeholder

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `repositories.rs` | Field names match SQL columns | Rename/removal |
| Services | All fields accessible | Type changes |

## Serde Usage

All models derive `Serialize` and `Deserialize` for:
- JSON storage of complex fields (agents, metadata)
- Wire format for gossip messages
- API response serialization

## Notes
- `Option<T>` fields map to nullable SQL columns
- Trust states: "trusted", "unknown", "unfollowed"
- Sync statuses track thread download progress
- Encrypted fields stored as `Vec<u8>`
