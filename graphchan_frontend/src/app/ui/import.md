# import.rs

## Purpose
Renders the thread import page for bringing external content into Graphchan. Supports importing threads from 4chan and Reddit, converting them to the native format with posts and attachments.

## Components

### `render_import_page`
- **Does**: Two-column layout with import forms for each supported platform
- **Interacts with**: `ImporterState`, `RedditImporterState`, import tasks

### 4chan Import Column
- **Does**: URL input and import button for 4chan threads
- **Interacts with**: `app.importer`, `tasks::import_fourchan`
- **Input**: 4chan thread URL (e.g., `https://boards.4chan.org/g/thread/12345`)

### Reddit Import Column
- **Does**: URL input and import button for Reddit threads
- **Interacts with**: `app.reddit_importer`, `tasks::import_reddit`
- **Input**: Reddit thread URL

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_import_page(app, ui)` for `ViewState::Import` | Signature change |
| `state.rs` | `ImporterState`, `RedditImporterState` with `url`, `importing`, `error` | Field removal |
| `tasks.rs` | `import_fourchan`, `import_reddit` functions | Function removal |

## Layout

```
Import Thread

┌─ 4chan ─────────────────┬─ Reddit ────────────────┐
│ Enter a 4chan thread    │ Enter a Reddit thread   │
│ URL to import it.       │ URL to import it.       │
│                         │                         │
│ URL: [______________]   │ URL: [______________]   │
│                         │                         │
│ [Import 4chan Thread]   │ [Import Reddit Thread]  │
└─────────────────────────┴─────────────────────────┘
```

## Import Flow

1. User enters thread URL
2. Clicks import button → `importing = true`, spinner shown
3. Backend fetches thread, converts to native format
4. Creates thread with posts, downloads images
5. Success → thread appears in catalog
6. Error → displayed in red below input

## Notes
- Import is async; UI remains responsive
- Error state cleared on retry
- Imported threads become local; original attribution preserved in post metadata
- Images downloaded and stored as local blobs
