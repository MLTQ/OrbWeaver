# importer.rs

## Purpose
Imports threads from external platforms (4chan, Reddit) into Graphchan. Supports initial import (full thread with posts and images) and incremental refresh (only new posts since last import). Uses `import_post_map` for deduplication across refreshes.

## Components

### `import_fourchan_thread(state, url, topics)`
- **Does**: Fetches a 4chan thread via JSON API, creates local thread + posts, downloads images, stores import mappings
- **Interacts with**: `ThreadService`, `FileService`, `TopicRepository`, `ImportPostMapRepository`, 4chan CDN
- **Flow**: Parse URL → fetch JSON → create thread → store topics → store source info → create posts with parent refs → download images → set last_refreshed → broadcast announcement

### `import_reddit_thread(state, url, topics)`
- **Does**: Same as 4chan import but for Reddit's nested comment structure
- **Interacts with**: Same services plus Reddit JSON API
- **Flow**: Fetch `.json` endpoint → create thread → store topics/source info → BFS through comment tree → download OP image if present → broadcast

### `refresh_thread(state, thread_id)`
- **Does**: Dispatcher that reads `source_url`/`source_platform` from thread record, calls platform-specific refresh
- **Interacts with**: `ThreadRepository` for source info lookup
- **Returns**: Updated `ThreadDetails`

### `refresh_fourchan_thread(state, thread_id, source_url)`
- **Does**: Re-fetches 4chan thread, skips already-imported posts via `import_post_map`, creates only new posts
- **Interacts with**: `ImportPostMapRepository`, `ThreadService`, `FileService`
- **Rationale**: Enables "live following" of 4chan threads that continue to get replies

### `refresh_reddit_thread(state, thread_id, source_url)`
- **Does**: Same as 4chan refresh but handles Reddit's nested tree structure
- **Interacts with**: Same services, BFS with parent tracking

### Helper Functions

#### `download_and_save_image` / `download_and_save_reddit_image`
- **Does**: Downloads image from source CDN, saves via `FileService`, publishes `FileAnnouncement`
- **Rate limiting**: 1500ms delay between 4chan image downloads to avoid 429 errors

#### `clean_body(html)`
- **Does**: Converts HTML to plain text via `html2text`, trims whitespace

#### `extract_references(html, id_map)`
- **Does**: Parses `>>12345` references from 4chan HTML, maps external post numbers to internal IDs
- **Rationale**: Preserves reply graph structure during import

#### `parse_thread_url(url)`
- **Does**: Regex extraction of board and thread ID from 4chan URLs

## Deduplication Strategy

The `import_post_map` table maps `(thread_id, external_id) → internal_id`:
- On initial import: every post gets an entry
- On refresh: `get_map()` loads existing entries, posts with known `external_id` are skipped
- Reddit uses comment IDs as `external_id`, 4chan uses post numbers

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `api/threads.rs` | `import_fourchan_thread`, `import_reddit_thread`, `refresh_thread` signatures | Parameter changes |
| `database/repositories` | `ImportPostMapRepository`, `ThreadRepository` traits in scope | Trait changes |
| `network.rs` | `publish_thread_announcement`, `publish_file_available` | Method changes |

## Notes
- Image-only posts use `"[image]"` placeholder body (backend requires non-empty body)
- Reddit importer prefixes author names: `**u/username**`
- 4chan import preserves original timestamps via `created_at` field
- Refresh only re-broadcasts if new posts were actually added
- The `topics` parameter is stored in `thread_topics` table; `publish_thread_announcement` reads from there automatically
