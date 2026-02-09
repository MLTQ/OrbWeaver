# state.rs

## Purpose
Defines all UI state structures for the frontend application. Central source of truth for view state, thread state, and layout data used across all UI modules.

## Components

### `ViewState`
- **Does**: Enum representing current top-level view (Catalog, Thread, Messages, etc.)
- **Interacts with**: Main app loop for view switching
- **Variants**: `Catalog`, `Messages`, `Thread(ThreadState)`, `Following`, `FollowingCatalog`, `Settings`, `Conversation`, `Blocking`, `SearchResults`

### `ThreadDisplayMode`
- **Does**: Enum for thread visualization mode selection
- **Interacts with**: All view modules check this to render appropriately
- **Variants**: `List` (default), `Graph`, `Chronological`, `Sugiyama`, `Radial`

### `ThreadState`
- **Does**: Complete state for viewing a thread including posts, layout, and navigation
- **Interacts with**: All view modules, input handler
- **Key fields**:
  - `details` - Thread content from API
  - `display_mode` - Current visualization
  - `graph_nodes`, `sugiyama_nodes`, `chrono_nodes`, `radial_nodes` - Per-view layout data
  - `selected_post`, `secondary_selected_post` - Navigation state
  - `graph_zoom`, `graph_offset` - Viewport state
  - `radial_rotation`, `radial_target_rotation` - Radial view animation

### `GraphNode`
- **Does**: Position and physics state for a post in force-directed layouts
- **Interacts with**: `graph.rs`, `sugiyama.rs`, `chronological.rs`
- **Fields**: `pos`, `vel`, `size`, `dragging`, `pinned`

### `RadialNode`
- **Does**: Ring and angular position for radial layout
- **Interacts with**: `radial.rs`, `input.rs`
- **Fields**: `ring` (depth), `angle` (radians), `size`

### `NavigationMode`
- **Does**: Tracks last navigation direction (Parent/Reply/None)
- **Interacts with**: VIM-style navigation in `input.rs`
- **Rationale**: Determines which cursor (parent vs reply) to show

### `IdentityState`
- **Does**: Local user identity and profile editing state
- **Interacts with**: Drawer, avatar cropper

### `ImportPlatform`
- **Does**: Enum selecting between `FourChan` and `Reddit` for the unified import dialog
- **Interacts with**: `ImporterState`, `dialogs.rs`

### `CreateThreadState`, `ImporterState`
- **Does**: Dialog state for thread creation and import
- **Interacts with**: `dialogs.rs`
- **Note**: `ImporterState` now handles both 4chan and Reddit via the `platform` field (previously had a separate `RedditImporterState`)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `input.rs` | `ThreadState` has `radial_nodes`, `selected_post`, `display_mode` | Field removal |
| `radial.rs` | `RadialNode` has `ring: usize`, `angle: f32` | Type changes |
| `graph.rs` | `GraphNode` has `pos`, `vel`, `pinned` | Field removal |
| All views | `ThreadDisplayMode` variants match render logic | Variant changes |

## Serialization

- Most fields serialize via serde for state persistence
- Physics state (`sim_start_time`, cursors) marked `#[serde(skip)]`
- Layout nodes serialize to preserve positions across sessions

## Notes
- `ThreadState::Default` sets sensible defaults (zoom=1.0, bin_seconds=60, etc.)
- Each view maintains its own node map to allow independent layout state
- Secondary selection used for showing relationships in graph views
