# importer.rs

## Purpose
Client-side helpers for importing threads from external platforms (4chan, Reddit). Delegates actual import work to the backend to avoid flooding the network with individual messages.

## Components

### `import_fourchan_thread`
- **Does**: Imports a 4chan thread via backend `/import` endpoint
- **Interacts with**: `ApiClient.import_thread`
- **Returns**: `Result<String>` (thread ID)

### `import_reddit_thread`
- **Does**: Imports a Reddit thread via backend `/import` endpoint
- **Interacts with**: `ApiClient.post_json`
- **Returns**: `Result<String>` (thread ID)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `tasks.rs` | Functions available for spawning | Function removal |
| `api.rs` | `import_thread`, `post_json` methods | Method removal |

## Import Flow

```
Frontend                    Backend
   │                           │
   │  POST /import {url}       │
   │ ─────────────────────────>│
   │                           │  Fetch from 4chan/Reddit
   │                           │  Create thread + posts
   │                           │  Download images
   │                           │  Broadcast ThreadSnapshot
   │  {id: "thread_id"}        │
   │ <─────────────────────────│
   │                           │
```

## Why Backend Import?

Previous approach (client-side import):
1. Client fetches thread from 4chan
2. Client creates thread via API
3. Client creates each post via API (N requests)
4. Each post triggers PostUpdate gossip message
5. **Problem**: Flooding network, timeouts, message loss

Current approach (backend import):
1. Client sends single import request
2. Backend handles everything internally
3. Backend broadcasts single ThreadSnapshot when complete
4. **Benefit**: Atomic, reliable, no network flooding

## Notes
- 4chan import uses `api.import_thread(url)` directly
- Reddit import uses `post_json` with explicit platform field
- Both return the created thread ID for navigation
- Errors propagated to UI for display
