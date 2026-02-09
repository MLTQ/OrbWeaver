# threading.rs

## Purpose
Business logic for thread and post management. Provides `ThreadService` for CRUD operations on threads and posts, including file attachment handling and topic association.

## Components

### `ThreadService`
- **Does**: Encapsulates all thread/post business logic
- **Interacts with**: `Database`, `ThreadRepository`, `PostRepository`, `FileRepository`, `TopicRepository`
- **Constructors**: `new(db)` or `with_file_paths(db, paths)` for file presence checking

### Thread Operations

#### `list_threads`
- **Does**: Lists recent threads with summaries including first image
- **Interacts with**: ThreadRepository, FileRepository for images, TopicRepository
- **Returns**: `Vec<ThreadSummary>` with metadata and first_image_file

#### `get_thread`
- **Does**: Fetches complete thread with posts and participating peers
- **Interacts with**: All repositories for full thread hydration
- **Returns**: `ThreadDetails` with posts, files, and peer info

#### `create_thread`
- **Does**: Creates new thread with initial post (OP)
- **Interacts with**: ThreadRepository, PostRepository
- **Returns**: Created thread details

#### `delete_thread`
- **Does**: Soft-deletes thread (sets deleted flag)
- **Interacts with**: ThreadRepository

### Post Operations

#### `create_post`
- **Does**: Creates post in thread with parent relationships
- **Interacts with**: PostRepository, validates parent_post_ids exist
- **Returns**: Created `PostView`

#### `list_recent_posts`
- **Does**: Lists recent posts across all threads (for activity feed)
- **Interacts with**: PostRepository with limit

### Data Types

#### `ThreadSummary`
- Lightweight thread info: id, title, creator, timestamps, topics, first_image
- Import fields: `source_url`, `source_platform`, `last_refreshed_at` (all optional, populated for imported threads)

#### `ThreadDetails`
- Full thread: summary + posts + participating peers

#### `PostView`
- Complete post: id, body, author, parents, files, metadata

#### `CreateThreadInput` / `CreatePostInput`
- Request payloads for creation

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api.rs` | `ThreadService` methods available | Method signature changes |
| `network/ingest.rs` | `create_thread`, `create_post` for gossip messages | Input type changes |

## File Presence Logic

When constructed `with_file_paths`:
- `FileView.present` set based on actual file existence
- Allows frontend to show "(remote)" vs available files
- Without paths, presence checking skipped

## Notes
- Uses `with_repositories` pattern for database access
- Post parent_post_ids validated to exist in same thread
- Topics stored in separate `thread_topics` table
- Thread creator auto-added to participating peers
