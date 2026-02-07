# handlers_threads.rs

## Purpose
Handles all async message responses related to threads and posts. These methods are called by the thin dispatcher in `messages.rs` when thread/post-related `AppMessage` variants arrive.

## Components

### `handle_threads_loaded`
- **Does**: Sorts threads by creation date, updates catalog, resolves `pending_thread_focus`
- **Interacts with**: `spawn_load_threads` triggers it, `open_thread` for focus

### `handle_thread_loaded`
- **Does**: Processes thread details for both initial load and refresh
- **Interacts with**: `ViewState::Thread`, graph/sugiyama/chronological nodes, image download pipeline
- **Rationale**: Most complex handler — differentiates refresh (incremental post merge, preserve viewport) from initial load (full state reset with graph rebuild)

### `handle_thread_created`
- **Does**: Resets create-thread form, switches view to new thread, triggers catalog refresh
- **Interacts with**: `CreateThreadState`, `build_initial_graph`, `spawn_load_threads`

### `handle_post_created`
- **Does**: Appends new post to thread details, adds graph node, triggers image downloads for attachments
- **Interacts with**: `ThreadState`, `GraphNode`, image pipeline

### `handle_post_attachments_loaded`
- **Does**: Processes file attachments for a post, triggers image downloads for present images
- **Interacts with**: `attachments` HashMap, `spawn_download_image`
- **Rationale**: Uses `return` (not `continue`) since this is a method, not inline match arm

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `messages.rs` | All 5 handler methods exist on `GraphchanApp` | Method removal/rename |
| `spawners.rs` | Handlers call `spawn_load_threads`, `spawn_load_image`, etc. | Spawner removal |
| `ui/graph.rs` | `build_initial_graph` called for initial thread load | Function signature |

## Notes
- `handle_thread_loaded` refresh path only clears `chronological_nodes` and `sugiyama_nodes` (not graph nodes) to preserve user-positioned nodes
- Image downloads collected into a `Vec` and spawned after releasing mutable borrows on `self.view`
- Peer cache (`self.peers`) updated from thread details on every load
