# mod.rs (database module)

## Purpose
SQLite database wrapper with schema migrations, connection management, and the repository pattern for data access. Provides thread-safe access via `Arc<Mutex<Connection>>`.

## Components

### `Database`
- **Does**: Thread-safe wrapper around SQLite connection
- **Fields**: `conn: Arc<Mutex<Connection>>`
- **Pattern**: Clone-able for sharing across threads

### `Database::new`
- **Does**: Opens/creates SQLite database and runs migrations
- **Flow**: Open connection → Run MIGRATIONS SQL → Return Database
- **Features**: WAL mode enabled, foreign keys enforced

### `with_repositories`
- **Does**: Provides repository access within a closure
- **Pattern**: `db.with_repositories(|repos| { repos.threads().get(...) })`
- **Rationale**: Ensures lock held for entire transaction

### `get_identity` / `set_identity`
- **Does**: Manages local node identity in `node_identity` table
- **Returns**: `(gpg_fingerprint, iroh_peer_id, friendcode)`

### `MIGRATIONS`
- **Does**: SQL schema definition and migrations
- **Contains**: All CREATE TABLE statements, indexes, triggers

## Schema Overview

### Core Tables
| Table | Purpose | Key |
|-------|---------|-----|
| `settings` | Key-value config | `key` |
| `node_identity` | Local identity (singleton) | `id=1` |
| `peers` | Known peers | `id` (GPG fingerprint) |
| `threads` | Discussion threads | `id` |
| `posts` | Individual messages | `id` |
| `post_relationships` | Multi-parent edges | `(parent_id, child_id)` |
| `files` | Attachments | `id` |

### Feature Tables
| Table | Purpose |
|-------|---------|
| `reactions` | Emoji reactions |
| `thread_tickets` | Iroh blob tickets for threads |
| `direct_messages` | Encrypted DMs |
| `conversations` | DM conversation metadata |
| `blocked_peers` | Direct blocks |
| `blocklist_subscriptions` | Subscribed blocklists |
| `blocklist_entries` | Entries in blocklists |
| `import_post_map` | Maps external post IDs to internal IDs for imported thread dedup |

### Indexes
- `idx_posts_thread` - Posts by thread_id
- `idx_post_relationships_child` - Post edges by child
- `idx_files_post` - Files by post_id

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| All services | `Database::new`, `with_repositories` | Method changes |
| `repositories.rs` | Schema matches repository SQL | Column changes |

## Notes
- WAL mode for concurrent reads
- Foreign keys with CASCADE deletes
- Thread-safe via Mutex (single writer)
- Migrations run on every startup (idempotent CREATE IF NOT EXISTS)
- `threads` table has import tracking columns: `source_url`, `source_platform`, `last_refreshed_at` (added by `ensure_import_tracking` migration)
