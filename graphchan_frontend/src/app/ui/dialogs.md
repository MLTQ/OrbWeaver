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
- **Does**: Modal window for importing a 4chan thread with URL input and topic selector
- **Interacts with**: `ImporterState`, `spawn_import_fourchan`
- **Controls**: `importer.open` boolean toggles visibility
- **Topic selector**: Same pattern as create thread dialog — checkboxes from `subscribed_topics`, stored in `importer.selected_topics`

## Layout (Import Dialog)

```
┌─ Import from 4chan ──────────────────────────┐
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
