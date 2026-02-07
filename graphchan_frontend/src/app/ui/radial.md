# radial.rs

## Purpose
Renders threads as a radial phylogenetic tree layout with the OP at center and replies arranged in concentric rings by depth. Inspired by evolutionary tree visualizations where distance from center represents conversation depth.

## Components

### `compute_ring_depths`
- **Does**: Assigns ring number to each post using longest-path algorithm
- **Interacts with**: Called by `build_radial_layout`
- **Rationale**: Uses max(parent depths) + 1 so multi-parent posts appear on the ring of their deepest parent, matching phylogenetic conventions

### `build_radial_layout`
- **Does**: Creates `RadialNode` entries with ring and angular positions for all posts
- **Interacts with**: `ThreadState.radial_nodes`, called when layout needs rebuilding
- **Rationale**: Posts sorted by `created_at` within each ring for consistent angular ordering

### `render_radial_view`
- **Does**: Main render function - draws ring guides, edges, and nodes
- **Interacts with**: `GraphchanApp`, `ThreadState`, `node::render_node`
- **Rationale**: Animates rotation smoothly via lerp toward `radial_target_rotation`

### `radial_to_screen`
- **Does**: Converts (ring, angle) to screen coordinates given center, zoom, and rotation
- **Interacts with**: Used throughout for positioning

### `angle_to_front`
- **Does**: Calculates rotation needed to bring a node to the bottom of the view
- **Interacts with**: Click handler for centering on clicked posts

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `input.rs` | `RadialNode` has `ring: usize` and `angle: f32` | Field removal/rename |
| `input.rs` | `radial_nodes` populated before navigation | Empty map handling |
| `state.rs` | `RadialNode` struct definition | Struct fields |
| `mod.rs` | `render_radial_view(app, ui, state)` signature | Function signature |

## Constants

- `RING_SPACING = 400.0` - Distance between concentric rings
- `CENTER_RADIUS = 200.0` - Radius of center area for OP
- `ROTATION_SPEED = 0.15` - Lerp factor for rotation animation

## Notes
- Ring guides drawn as dashed circles for visual hierarchy
- Edges curve toward center (control point at midpoint pulled inward)
- Rotation only resets on first initialization, not when posts change
