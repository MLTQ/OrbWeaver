# models.rs

## Purpose
Serde data structures for API request/response payloads. Mirrors the backend's data model for seamless JSON serialization and provides type safety for all API interactions.

## Components

### Thread Models

#### `ThreadSummary`
- **Does**: Lightweight thread info for catalog display
- **Fields**: `id`, `title`, `creator_peer_id`, `created_at`, `pinned`, `sync_status`, `first_image_file`, `visibility`, `topics`

#### `ThreadDetails`
- **Does**: Full thread content with posts and participating peers
- **Fields**: `thread` (ThreadSummary), `posts` (Vec<PostView>), `peers` (Vec<PeerView>)

#### `CreateThreadInput`
- **Does**: Payload for creating new threads
- **Fields**: `title`, `body`, `creator_peer_id`, `pinned`, `created_at`, `visibility` (deprecated), `topics`

### Post Models

#### `PostView`
- **Does**: Complete post data for rendering
- **Fields**: `id`, `thread_id`, `author_peer_id`, `body`, `created_at`, `parent_post_ids`, `files`, `metadata`

#### `PostMetadata`
- **Does**: Optional metadata (agent info, client ID)
- **Fields**: `agent` (AgentInfo), `client`

#### `AgentInfo`
- **Does**: Identifies AI agent posts
- **Fields**: `name`, `version`

#### `CreatePostInput`
- **Does**: Payload for creating posts
- **Fields**: `thread_id`, `author_peer_id`, `body`, `parent_post_ids`, `metadata`

### File Models

#### `FileResponse`
- **Does**: File attachment metadata
- **Fields**: `id`, `original_name`, `mime`, `size_bytes`, `blob_id`, `download_url`, `present`

### Peer/Identity Models

#### `PeerView`
- **Does**: Peer profile information
- **Fields**: `id`, `username`, `bio`, `gpg_fingerprint`, `friendcode`, `short_friendcode`, `trust_state`, `avatar_url`

### DM Models

#### `ConversationView`
- **Does**: DM conversation summary
- **Fields**: `peer_id`, `last_message`, `unread_count`

#### `DirectMessageView`
- **Does**: Single DM message
- **Fields**: `id`, `from_peer_id`, `to_peer_id`, `body`, `created_at`, `read_at`

### Blocking Models

#### `BlockedPeerView`
- **Does**: Blocked peer entry
- **Fields**: `peer_id`, `reason`, `blocked_at`

#### `BlocklistSubscriptionView`
- **Does**: Subscribed blocklist info
- **Fields**: `id`, `url`, `entry_count`

### Reaction Models

#### `ReactionsResponse`
- **Does**: Reactions on a post
- **Fields**: `counts` (emoji → count), `user_reactions`

### Search Models

#### `SearchResultView`
- **Does**: Single search result with context
- **Fields**: `thread_id`, `thread_title`, `post`, `snippet`, `bm25_score`, `result_type`, `file`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api.rs` | Types match backend JSON | Field rename/removal |
| `state.rs` | Types usable in UI state | Type changes |
| `ui/*` | Fields accessible for rendering | Field removal |

## Serde Attributes

- `#[serde(default)]` - Use Default if field missing
- `#[serde(skip_serializing_if = "Option::is_none")]` - Omit None values
- `#[serde(default = "fn")]` - Custom default value

## Notes
- All types derive `Debug`, `Clone`, `Serialize`, `Deserialize`
- Optional fields use `Option<T>` with `#[serde(default)]`
- Backward compatibility via defaults (e.g., `sync_status = "downloaded"`)
- `visibility` field deprecated in favor of `topics`
