# thread.rs

## Purpose
Renders threads in traditional linear list view and provides the main thread viewing container. Also handles thread opening logic and shared utilities like post body rendering with link parsing.

## Components

### `ThreadAction`
- **Does**: Enum for actions returned from thread rendering (None, GoBack, OpenThread)
- **Interacts with**: Main app loop for navigation

### `render_post_body`
- **Does**: Renders post text with support for `>>>thread_id` cross-links
- **Interacts with**: Used by list view and `node.rs` for graph views
- **Rationale**: Centralized to ensure consistent link handling across all views

### `GraphchanApp::open_thread`
- **Does**: Transitions app state to viewing a thread, initializes ThreadState
- **Interacts with**: `ViewState::Thread`, spawns `load_thread` task
- **Rationale**: Single entry point for all thread opening (catalog click, link click, etc.)

### `render_reactions`
- **Does**: Renders emoji reaction bar for a post
- **Interacts with**: `ThreadState.reactions`, backend API for loading

### `render_thread_list_view`
- **Does**: Main list view renderer - posts as full-width cards in chronological order
- **Interacts with**: `ThreadState`, calls into graph/chronological/radial views based on display_mode

### `render_post_card`
- **Does**: Renders a single post as a card with header, body, attachments, and actions
- **Interacts with**: `render_post_body`, file attachment rendering

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `node.rs` | `render_post_body(ui, body)` returns `Option<String>` | Return type |
| `catalog.rs` | `open_thread(summary)` accepts `ThreadSummary` | Parameter type |
| `mod.rs` | Module exports `ThreadAction`, `render_post_body` | Export removal |

## Post Body Parsing

- Plain text rendered as labels
- `>>>thread_id` rendered as clickable links (cross-thread navigation)
- Lines processed individually with tight spacing (2.0px) matching 4chan style

## Notes
- List view is the default/fallback when display_mode is Posts
- Floating draft composer window rendered separately
- Thread loading is async via message channel
