# dialogs.rs

## Purpose
Renders modal dialog windows for thread creation and other popup interactions. Provides forms for creating new threads with titles, bodies, attachments, and topic selection.

## Components

### `render_create_thread_dialog`
- **Does**: Modal window for creating a new thread
- **Interacts with**: `CreateThreadState`, `tasks::pick_files`, `tasks::create_thread`
- **Controls**: `show_create_thread` boolean toggles visibility

### Form Fields
- **Title**: Single-line text input (required)
- **Body**: Multiline text area with hint text (optional)
- **Attachments**: File list with "Attach Files" button triggering native picker

### Topic Selector
- **Does**: Checkboxes for selecting which topics to announce thread on
- **Interacts with**: `subscribed_topics`, `selected_topics`
- **Behavior**: No topics selected = friends-only thread (warning shown)

### Submit Handling
- **Does**: Validates and submits thread creation
- **Interacts with**: `tasks::create_thread_with_files`
- **States**: `submitting` shows spinner, `error` shows error message

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_create_thread_dialog(ctx)` called each frame | Signature change |
| `state.rs` | `CreateThreadState` has `title`, `body`, `files`, `submitting` | Field removal |
| `tasks.rs` | `pick_files`, `create_thread_with_files` available | Function removal |

## Layout

```
┌─ Create Thread ─────────────────────────────┐
│ Title                                       │
│ [________________________________]          │
│                                             │
│ Body (optional)                             │
│ [Write the opening post...          ]       │
│ [                                   ]       │
│                                             │
│ Attachments: file1.png, file2.jpg           │
│ [Attach Files]                              │
│                                             │
│ ┌─ 📡 Announce to Topics ─────────────────┐ │
│ │ [x] tech                                │ │
│ │ [ ] art                                 │ │
│ │ ✓ Will announce to 1 topic(s)          │ │
│ └─────────────────────────────────────────┘ │
│                                             │
│ [Create] [Cancel]                           │
└─────────────────────────────────────────────┘
```

### `render_import_dialog`
- **Does**: Modal window for importing threads from 4chan or Reddit, with platform selector, URL input, and topic selector
- **Interacts with**: `ImporterState`, `ImportPlatform`, `spawn_import_fourchan`, `spawn_import_reddit`
- **Controls**: `importer.open` boolean toggles visibility, `importer.platform` selects 4chan vs Reddit
- **Topic selector**: Same pattern as create thread dialog — checkboxes from `subscribed_topics`, stored in `importer.selected_topics`
- **Rationale**: Consolidated from a separate full-page Import view (`import.rs`, now deleted) and a separate `RedditImporterState` into a single unified dialog

## Layout (Import Dialog)

```
┌─ Import Thread ──────────────────────────────┐
│ Platform: [4chan] [Reddit]                    │
│                                              │
│ Paste a thread URL (e.g. https://...)        │
│ [________________________________]           │
│                                              │
│ ┌─ Announce to Topics ──────────────────────┐│
│ │ [x] tech                                 ││
│ │ [ ] art                                  ││
│ │ Will announce to 1 topic(s)              ││
│ └──────────────────────────────────────────┘│
│                                              │
│ [Import] [Close]                             │
└──────────────────────────────────────────────┘
```

## Notes
- Dialogs anchored to center of screen
- Topic Manager can be opened from within dialog if no topics subscribed
- Files picked via native OS dialog (`rfd` crate)
