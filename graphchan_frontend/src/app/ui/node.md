# node.rs

## Purpose
Shared node rendering logic used by all graph-based views (Graph, Timeline, Hierarchical, Radial). Provides consistent post card appearance and interaction behavior across visualizations.

## Components

### `NodeLayoutData`
- **Does**: Bundles post data with layout rect and attachments for rendering
- **Interacts with**: All graph views construct this before calling `render_node`

### `ImagePreview`
- **Does**: Enum representing image load state (Ready/Loading/Error/None)
- **Interacts with**: `image_preview` function, used for thumbnail rendering

### `image_preview`
- **Does**: Returns current image state, triggering download if needed
- **Interacts with**: `GraphchanApp.image_textures`, `image_loading`, `image_pending`, `image_errors`
- **Rationale**: Lazy loading with state tracking prevents duplicate downloads

### `render_node`
- **Does**: Renders a post as an interactive card with header, body, attachments, and actions
- **Interacts with**: `GraphchanApp`, `ThreadState`, `thread::render_post_body`
- **Rationale**: Centralized to ensure visual consistency across all graph views

### `estimate_node_size`
- **Does**: Estimates pixel dimensions for a post card based on content
- **Interacts with**: Layout functions in all graph views
- **Rationale**: Pre-calculation needed for layout algorithms before rendering

### `is_image`
- **Does**: Returns true if a `FileResponse` is an image MIME type
- **Interacts with**: Used throughout for conditional thumbnail rendering

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `graph.rs` | `render_node` returns `egui::Response` | Return type |
| `radial.rs` | `estimate_node_size(ui, post, has_preview, children_count)` signature | Parameters |
| `chronological.rs` | `NodeLayoutData` has `post`, `rect`, `attachments` fields | Struct fields |
| `sugiyama.rs` | `is_image(file)` checks MIME type | Logic change |

## Visual Design

- **Fill colors**: Dark gray default, yellow tint when selected, blue tint for reply targets
- **Stroke colors**: Reaction-based hue, yellow when selected, white for secondary selection
- **Layout**: Header (author + time), body (markdown), attachments, action buttons
- **Scaling**: Font sizes and spacing scale with zoom factor

## Notes
- Reaction colors calculated from `reaction_colors.rs` module
- Pin button shown on hover for graph/sugiyama views
- Truncates long posts with "..." and scroll handling
