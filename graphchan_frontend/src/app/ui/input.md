# input.rs

## Purpose
Centralized keyboard navigation handler for all thread views. Provides consistent navigation controls across List, Graph, Timeline, Hierarchical, and Radial views while adapting to each view's spatial layout.

## Components

### `handle_keyboard_input`
- **Does**: Main entry point called every frame to process keyboard input
- **Interacts with**: `GraphchanApp`, `ThreadState`, all view modules via `center_viewport_on_post`
- **Rationale**: Centralized to ensure consistent behavior and avoid duplicate key handling

### `center_viewport_on_post`
- **Does**: Adjusts `graph_offset` to center viewport on a given post ID
- **Interacts with**: `ThreadState.graph_offset`, view-specific node maps (`graph_nodes`, `sugiyama_nodes`, `radial_nodes`, `chrono_nodes`)
- **Rationale**: Each view calculates position differently; this function handles the dispatch

### `consume_key`
- **Does**: Checks if a key+modifier combo was pressed and consumes the event
- **Interacts with**: egui input system
- **Rationale**: Prevents keys from being handled multiple times

### `is_text_edit_focused`
- **Does**: Returns true if a TextEdit widget has focus (draft window)
- **Interacts with**: egui focus system
- **Rationale**: Disables navigation when user is typing

### `should_auto_select_op`
- **Does**: Returns true if a navigation key was pressed with no selection
- **Interacts with**: Used to auto-select OP on first keypress

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` | `handle_keyboard_input(app, ctx)` called each frame | Signature change |
| `radial.rs` | Uses `RING_SPACING`, `CENTER_RADIUS` constants for centering | Constant value changes |
| `state.rs` | `ThreadState` has `selected_post`, `graph_offset`, `display_mode` | Field removal |

## Key Bindings

### Global
- **Tab/Shift+Tab**: Next/previous post chronologically
- **Space**: Re-center on selected post
- **Escape**: Deselect

### Radial View
- **Left/Right**: Move between posts on same ring
- **Up/Down**: Move between rings (finds angularly closest post)

### VIM-style (all graph views)
- **U/P**: Cycle parent cursor
- **I**: Select parent at cursor
- **J/;**: Cycle reply cursor
- **K**: Select reply at cursor

## Notes
- Surrenders egui focus on Tab to prevent widget cycling
- Radial navigation builds `posts_by_ring` map for efficient ring traversal
- Constants duplicated from radial.rs for centering calculation (TODO: extract to shared module)
