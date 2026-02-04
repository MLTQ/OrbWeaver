# catalog.rs

## Purpose
Renders the thread catalog view - a grid of thread summaries showing available discussions. Provides filtering by topic, visibility toggles, and three-column layout separating local threads from network threads.

## Components

### `render_catalog`
- **Does**: Main catalog renderer with topic filters and thread grid
- **Interacts with**: `GraphchanApp.threads`, `catalog_topic_filter`, `subscribed_topics`

### `render_catalog_three_columns`
- **Does**: Three-column layout: My Threads | Network Threads | Recent Posts
- **Interacts with**: Thread lists partitioned by `sync_status`

### Topic Filtering
- **Does**: Filter chips for subscribed topics, "All Topics" default
- **Interacts with**: `catalog_topic_filter`, `show_topic_manager`

### Visibility Toggles
- **Does**: Checkboxes for ignored threads and global threads
- **Interacts with**: `show_ignored_threads`, `show_global_threads`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_catalog(ui)` called when `ViewState::Catalog` | Signature change |
| `thread.rs` | Calls `open_thread(summary)` on thread click | Method availability |
| `topics.rs` | `show_topic_manager` controls topic dialog | Field removal |

## Layout

```
[Topic chips: All Topics | Topic1 | Topic2 | + Manage Topics]
[x] Show ignored threads    [x] Show global threads

| My Threads      | Network Threads  | Recent Posts    |
| (downloaded)    | (available)      | (activity feed) |
```

## Thread Partitioning

- **My Threads**: `sync_status == "downloaded"` (locally stored)
- **Network Threads**: All other threads (available on network)
- Filtered by topic and visibility settings before partitioning

## Notes
- Thread cards show title, post count, last activity
- Click thread card to open via `open_thread`
- Global threads from all Graphchan users hidden by default
