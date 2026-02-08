# events.rs

## Purpose
Defines gossip message types and handles outbound event publishing. Contains the wire format for all P2P communication including thread announcements, file sharing, and profile updates.

## Components

### `EventEnvelope`
- **Does**: Top-level wrapper for all gossip messages
- **Fields**: version (u8), topic (String), payload (EventPayload)
- **Rationale**: Version field allows protocol evolution

### `EventPayload`
- **Does**: Enum of all message types
- **Variants**: ThreadAnnouncement, PostUpdate, FileAvailable, FileRequest, FileChunk, ProfileUpdate, ReactionUpdate, DirectMessage, BlockAction

## Message Types

### `ThreadAnnouncement`
- **Does**: Announces thread existence with download ticket
- **Fields**: thread_id, creator/announcer peer IDs, title, preview, ticket, post_count, thread_hash, topics
- **Use case**: Topic-based discovery, sync detection via hash

### `PostUpdate` (uses `PostView`)
- **Does**: Single post update (creation or edit)
- **Use case**: Real-time post propagation

### `FileAnnouncement`
- **Does**: Announces file availability with blob ticket
- **Fields**: id, post_id, thread_id, original_name, mime, size, ticket
- **Use case**: P2P file sharing without centralized storage

### `FileRequest` / `FileChunk`
- **Does**: Legacy file transfer protocol (deprecated)
- **Note**: Replaced by Iroh blob tickets but kept for compatibility

### `ProfileUpdate`
- **Does**: Peer profile changes (username, bio, avatar, X25519 key)
- **Fields**: peer_id, username, bio, avatar_file_id, agents, x25519_pubkey
- **Use case**: Profile sync across network; X25519 key propagation enables DMs with short-friendcode peers

### `ReactionUpdate`
- **Does**: Emoji reaction add/remove
- **Fields**: post_id, emoji, reactor_peer_id, action (add/remove), signature

### `DirectMessageEvent`
- **Does**: Encrypted DM delivery via gossip
- **Fields**: from_peer_id, to_peer_id, encrypted_body, nonce, message_id, conversation_id, created_at
- **Routing**: `peer-{to_peer_id}` topic (point-to-point, not re-broadcast)

### `BlockActionEvent`
- **Does**: Block/unblock action broadcast for shared blocklist features
- **Fields**: blocker_peer_id, blocked_peer_id, reason, is_unblock
- **Routing**: `peer-{blocker_peer_id}` topic (subscribers with auto_apply receive and enforce)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ingest.rs` | Can deserialize all `EventPayload` variants | Variant changes |
| `network.rs` | Can serialize events for broadcast | Field changes |

## Wire Format

Messages serialized as JSON within gossip packets:
```json
{
  "version": 1,
  "topic": "graphchan-global-v1",
  "payload": {
    "ThreadAnnouncement": {
      "thread_id": "...",
      "ticket": "blob:...",
      ...
    }
  }
}
```

## Notes
- FileChunk uses base64 encoding for binary data in JSON
- `visibility` field deprecated in favor of `topics` array
- `thread_hash` enables sync detection without full content comparison
- Announcer may differ from creator (re-sharing)
