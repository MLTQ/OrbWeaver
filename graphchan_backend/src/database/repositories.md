# repositories.rs

## Purpose
Repository traits and implementations defining all database operations. Follows the repository pattern to abstract SQL queries behind typed interfaces.

## Components

### Core Repository Traits

#### `ThreadRepository`
- `create`, `upsert`, `get`, `list_recent`
- `set_rebroadcast`, `should_rebroadcast`
- `delete`, `set_ignored`, `is_ignored`

#### `PostRepository`
- `create`, `upsert`, `get`
- `list_for_thread`, `list_recent`
- `add_relationships`, `parents_of`, `has_children`

#### `PeerRepository`
- `upsert`, `get`, `list`, `delete`

#### `FileRepository`
- `attach`, `upsert`, `get`
- `list_for_post`, `list_for_thread`

#### `ReactionRepository`
- `add`, `remove`
- `list_for_post`, `count_for_post`

### DM Repository Traits

#### `DirectMessageRepository`
- `create`, `get`
- `list_for_conversation`
- `mark_as_read`, `count_unread`

#### `ConversationRepository`
- `upsert`, `get`, `list`
- `update_unread_count`, `update_last_message`

### Blocking Repository Traits

#### `BlockedPeerRepository`
- `block`, `unblock`, `is_blocked`, `list`

#### `BlocklistRepository`
- `subscribe`, `unsubscribe`, `list_subscriptions`
- `add_entry`, `remove_entry`, `list_entries`
- `is_in_any_blocklist`

#### `IpBlockRepository`
- `add`, `remove`, `set_active`
- `increment_hit_count`, `list_active`

### Utility Repository Traits

#### `SearchRepository`
- `search(query, limit)` - Full-text search across posts/files

#### `PeerIpRepository`
- `update`, `get`, `get_by_ip`, `get_ips`, `list_all`

#### `TopicRepository`
- `subscribe`, `unsubscribe`, `list_subscribed`
- `add_thread_topic`, `list_thread_topics`

### `Repositories` Struct
- **Does**: Bundles all repository implementations
- **Pattern**: Created per-transaction via `Database::with_repositories`
- **Access**: `repos.threads()`, `repos.posts()`, etc.

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| Services | Trait methods available | Method removal |
| `mod.rs` | `Repositories::new(conn)` | Constructor change |

## Pattern

```rust
// Service usage
self.database.with_repositories(|repos| {
    let thread = repos.threads().get(thread_id)?;
    let posts = repos.posts().list_for_thread(thread_id)?;
    // ... business logic
    Ok(result)
})
```

## Implementation Notes

- Uses `rusqlite` with `params!` macro
- `OptionalExtension` for nullable results
- Transactions via closure (lock held)
- `upsert` = INSERT OR REPLACE semantics

## Notes
- Traits enable testing with mock implementations
- All methods return `Result<T>` for error handling
- Composite keys use tuple parameters
- `HashMap` returns for aggregations (e.g., reaction counts)
