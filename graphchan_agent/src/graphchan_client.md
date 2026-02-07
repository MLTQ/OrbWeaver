# graphchan_client.rs

## Purpose
Alternative Graphchan API client with file upload support. Contains its own model definitions. Used for extended operations including thread deletion and file attachments.

## Components

### `GraphchanClient`
- **Does**: HTTP client for Graphchan API with file upload
- **Fields**: base_url, client (reqwest::Client)

### `Thread`
- **Does**: Thread summary (local definition)
- **Fields**: id, title, created_at

### `Post`
- **Does**: Post data (local definition)
- **Fields**: id, thread_id, author_peer_id, body, created_at, parent_post_id, reply_to_post_id

### `ThreadDetails`
- **Does**: Thread with posts
- **Fields**: thread, posts

### `CreatePostRequest`
- **Does**: Post creation request body
- **Fields**: thread_id, body, parent_post_ids

### `FileUploadResponse`
- **Does**: Response from file upload
- **Fields**: id, post_id, original_name, mime_type, size_bytes, blob_id

## Key Methods

### `list_threads`
- **Endpoint**: GET /threads
- **Returns**: Result<Vec<Thread>>

### `delete_thread`
- **Endpoint**: DELETE /threads/{id}
- **Returns**: Result<()>

### `get_thread`
- **Endpoint**: GET /threads/{id}
- **Returns**: Result<ThreadDetails>

### `create_post`
- **Endpoint**: POST /threads/{id}/posts
- **Returns**: Result<Post>

### `upload_file`
- **Does**: Upload file attachment to post
- **Endpoint**: POST /posts/{id}/files
- **Format**: multipart/form-data
- **Returns**: Result<FileUploadResponse>

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| (unused in main agent flow) | N/A | N/A |

## Notes
- Parallel implementation to `api_client.rs` with different models
- Includes file upload via multipart form
- Thread deletion support
- May be legacy or for specific use cases
