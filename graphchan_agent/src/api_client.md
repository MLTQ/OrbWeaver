# api_client.rs

## Purpose
HTTP client for the Graphchan REST API. Used by the agent to fetch threads, posts, and create replies. This is the primary API client used in the agent loop.

## Components

### `GraphchanClient`
- **Does**: Wrapper around reqwest for Graphchan API
- **Fields**: base_url, client (reqwest::Client)

### `GraphchanClient::list_threads`
- **Does**: Get all threads
- **Endpoint**: GET /threads
- **Returns**: Result<Vec<ThreadSummary>>

### `GraphchanClient::get_thread`
- **Does**: Get thread with all posts
- **Endpoint**: GET /threads/{id}
- **Returns**: Result<ThreadDetails>

### `GraphchanClient::get_recent_posts`
- **Does**: Get recent posts across all threads
- **Endpoint**: GET /posts/recent?limit={n}
- **Returns**: Result<RecentPostsResponse>

### `GraphchanClient::create_post`
- **Does**: Create a new post/reply
- **Endpoint**: POST /threads/{id}/posts
- **Returns**: Result<PostView>

### `GraphchanClient::health_check`
- **Does**: Verify API is reachable
- **Endpoint**: GET /threads
- **Returns**: Result<()>

### `GraphchanClient::get_self_peer`
- **Does**: Get agent's own peer identity
- **Endpoint**: GET /peers/self
- **Returns**: Result<PeerView>

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/mod.rs` | All methods | Signature changes |
| `main.rs` | `GraphchanClient::new(url)` | Constructor |

## Notes
- Uses models from `models.rs` for request/response types
- Error handling includes response body for debugging
- Base URL configurable via `AgentConfig.graphchan_api_url`
