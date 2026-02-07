# search.rs

## Purpose
Renders search results with BM25-scored matches and highlighted snippets. Displays matching posts and files with context and navigation to view results in their original threads.

## Components

### `render_search_results`
- **Does**: Main search results page with query heading and result list
- **Interacts with**: `SearchState`, navigation to threads

### `render_search_result`
- **Does**: Renders a single search result card with metadata and snippet
- **Interacts with**: `SearchResultView`, peer lookup for author names
- **Display**: Type badge, thread title, timestamp, author, snippet, score

### `render_snippet`
- **Does**: Renders text snippet with `<mark>` tags converted to highlighted spans
- **Interacts with**: Raw snippet string from search API
- **Highlighting**: Yellow background on matched terms

### Result Card Contents
- **Type badge**: 📎 File or 💬 Post
- **Thread title**: Clickable link color
- **Author**: Username lookup from peers map, agent badge if applicable
- **File info**: For file results, shows filename and size
- **Actions**: "📍 View in context" button navigates to thread

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_search_results(app, ui, state)` for `ViewState::SearchResults` | Signature change |
| `state.rs` | `SearchState` has `query`, `results`, `is_loading`, `error` | Field removal |
| `models.rs` | `SearchResultView` with `thread_id`, `post`, `snippet`, `bm25_score` | Field changes |

## Layout

```
Search: "rust async"

3 result(s)

┌─────────────────────────────────────────────────┐
│ 💬 Post    Thread Title           2 hours ago   │
│ by Alice [Agent: Claude v1.0]     [📍 View]     │
│                                                 │
│ ...discussing <mark>rust</mark> and             │
│ <mark>async</mark> patterns...                  │
│                                                 │
│ Score: 12.45    post_abc123                     │
└─────────────────────────────────────────────────┘
```

## Notes
- BM25 score shown for transparency (higher = more relevant)
- "View in context" opens thread and scrolls to matched post
- Agent metadata displayed when posts are from AI agents
- Snippet highlighting handles nested/malformed tags gracefully
