# import_post_map.rs

## Purpose
Repository for the `import_post_map` table, which maps external post IDs (4chan post numbers, Reddit comment IDs) to internal Graphchan post IDs. Enables deduplication during thread refresh so already-imported posts are skipped.

## Components

### `SqliteImportPostMapRepository`
- **Does**: SQLite implementation of `ImportPostMapRepository` trait
- **Interacts with**: `import_post_map` table, `Connection` from parent `SqliteRepositories`

### `insert(thread_id, external_id, internal_id)`
- **Does**: Stores a mapping from external ID to internal post ID for a given thread
- **SQL**: `INSERT OR IGNORE` — silently skips duplicates
- **Rationale**: `OR IGNORE` makes it safe to call multiple times for the same mapping

### `get_map(thread_id)`
- **Does**: Loads all external→internal mappings for a thread as a `HashMap<String, String>`
- **Interacts with**: Used by refresh functions in `importer.rs` to determine which posts are new

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `importer.rs` | `insert` and `get_map` available via `repos.import_post_map()` | Method removal |
| `repositories/mod.rs` | `SqliteImportPostMapRepository` struct with `conn` field | Struct changes |
| `database/mod.rs` | `import_post_map` table exists (created by migration) | Schema changes |

## Notes
- Table has composite primary key `(thread_id, external_id)` with CASCADE deletes on both FKs
- External IDs are stored as strings (4chan post numbers converted from u64, Reddit comment IDs are already strings)
