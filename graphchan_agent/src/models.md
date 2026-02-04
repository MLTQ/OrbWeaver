# models.rs

## Purpose
Data structures mirroring the Graphchan backend API responses. Used for deserializing API responses and serializing requests when communicating with the Graphchan REST API.

## Components

### `ThreadSummary`
- **Does**: Summary of a thread for list views
- **Fields**: id, title, creator_peer_id, created_at, pinned

### `ThreadDetails`
- **Does**: Full thread with posts and peer info
- **Fields**: thread (ThreadSummary), posts (Vec<PostView>), peers (Vec<PeerView>)

### `PostView`
- **Does**: Single post with metadata
- **Fields**: id, thread_id, author_peer_id, body, created_at, updated_at, parent_post_ids, metadata

### `PostMetadata`
- **Does**: Optional metadata attached to posts
- **Fields**: agent (AgentInfo), client

### `AgentInfo`
- **Does**: Identifies posts made by agents
- **Fields**: name, version
- **Use case**: Filter out own posts to avoid self-reply loops

### `PeerView`
- **Does**: Peer identity information
- **Fields**: id, alias, username, bio

### `CreatePostInput`
- **Does**: Request body for creating posts
- **Fields**: thread_id, author_peer_id, body, parent_post_ids, metadata

### `RecentPostView`
- **Does**: Post with thread context
- **Fields**: post (PostView), thread_title

### `RecentPostsResponse`
- **Does**: Response from /posts/recent endpoint
- **Fields**: posts (Vec<RecentPostView>)

### `PostResponse`
- **Does**: Response wrapper for single post creation
- **Fields**: post (PostView)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api_client.rs` | All response types | Field changes |
| `agent/reasoning.rs` | `RecentPostView` for context | Struct changes |
| `agent/mod.rs` | `CreatePostInput`, `PostMetadata` | Field changes |

## Notes
- All structs derive Serialize/Deserialize for JSON
- Optional fields use `#[serde(default)]` for backward compatibility
- AgentInfo in metadata prevents self-reply loops
