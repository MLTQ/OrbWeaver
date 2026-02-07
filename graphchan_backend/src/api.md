# api.rs

## Purpose
Axum-based REST API server exposing all Graphchan functionality via HTTP endpoints. Handles request routing, multipart uploads, CORS, and delegates to service layer for business logic.

## Components

### `AppState`
- **Does**: Shared state passed to all handlers via Axum's State extractor
- **Fields**: `config`, `identity`, `database`, `network`, `blobs`, `http_client`
- **Pattern**: Clone-able for concurrent handler access

### `serve_http`
- **Does**: Starts HTTP server with all routes configured
- **Interacts with**: `find_available_port`, route definitions, `AppState`
- **Features**: CORS enabled (Any origin), body limit configured

### `find_available_port`
- **Does**: Tries start_port, increments up to 100 times to find available port
- **Rationale**: Allows multiple instances without port conflicts

## Route Categories

### Threads (`/threads`)
- `GET /threads` - List recent threads
- `POST /threads` - Create thread with optional files
- `GET /threads/:id` - Get thread with posts and peers
- `POST /threads/:id/posts` - Create post in thread
- `POST /threads/:id/download` - Trigger P2P download
- `DELETE /threads/:id` - Delete thread
- `POST /threads/:id/ignore` - Toggle ignored flag

### Posts (`/posts`)
- `GET /posts/recent` - List recent posts across threads
- `GET /posts/:id/files` - List post attachments
- `POST /posts/:id/files` - Upload file to post
- `GET /posts/:id/reactions` - Get reactions
- `POST /posts/:id/react` - Add reaction
- `POST /posts/:id/unreact` - Remove reaction

### Files (`/files`, `/blobs`)
- `GET /files/:id` - Download file by ID
- `GET /blobs/:blob_id` - Download via Iroh blob hash

### Identity (`/identity`, `/peers`)
- `GET /peers/self` - Get local identity
- `POST /identity/profile` - Update username/bio
- `POST /identity/avatar` - Upload avatar
- `GET /peers` - List followed peers
- `POST /peers` - Add peer via friendcode
- `DELETE /peers/:id` - Unfollow peer

### DMs (`/dms`)
- `GET /dms/conversations` - List conversations
- `POST /dms/send` - Send encrypted DM
- `GET /dms/:peer_id/messages` - Get conversation history
- `GET /dms/unread/count` - Unread count

### Blocking (`/blocking`)
- `GET /blocking/peers` - List blocked peers
- `POST /blocking/peers/:id` - Block peer
- `DELETE /blocking/peers/:id` - Unblock
- Blocklist management endpoints

### Topics
- `GET /topics` - List subscribed topics
- `POST /topics/:id/subscribe` - Subscribe
- `POST /topics/:id/unsubscribe` - Unsubscribe

### Search & Import
- `GET /search` - Full-text search
- `POST /import` - Import 4chan/Reddit thread

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| Frontend `api.rs` | Endpoint paths and response shapes | Path/response changes |
| Agent | Same endpoints as frontend | Breaking changes |

## Notes
- Uses `DefaultBodyLimit` for upload size limits
- CORS allows any origin (development friendly)
- Handlers return `Result<Json<T>, StatusCode>` or streaming Response
- Multipart handling for file uploads
