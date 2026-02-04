# sugiyama.rs

## Purpose
Renders threads as a layered hierarchical graph using Sugiyama-style layout. Posts are arranged in horizontal layers by depth, with edges flowing left-to-right. Optimizes for minimal edge crossings and clear visual hierarchy.

## Components

### `build_sugiyama_layout`
- **Does**: Assigns layer (X) and position (Y) to each post using BFS ranking
- **Interacts with**: `ThreadState.sugiyama_nodes`, post sizes for spacing
- **Rationale**: Layer = max(parent layers) + 1 ensures replies appear after all parents

### `render_sugiyama_view`
- **Does**: Main render function - draws edges and nodes in hierarchical arrangement
- **Interacts with**: `GraphchanApp`, `ThreadState`, `node::render_node`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `input.rs` | `sugiyama_nodes` map with `GraphNode.pos` | Map type change |
| `state.rs` | Reuses `GraphNode` struct from graph view | Struct changes |
| `mod.rs` | `render_sugiyama_view(app, ui, state)` signature | Function signature |

## Algorithm Details

1. **Rank Assignment** (BFS):
   - Roots (no parents) get rank 0
   - Each post gets rank = max(parent ranks) + 1
   - If no roots found, oldest post becomes rank 0

2. **Layer Ordering**:
   - Posts within each layer sorted by average parent position
   - Reduces edge crossings heuristically

3. **Coordinate Assignment**:
   - X = layer * layer_spacing
   - Y = distributed within layer based on node heights
   - Respects actual node sizes for proper spacing

## Constants

- `NODE_WIDTH = 300.0` - Default card width
- `LAYER_HEIGHT = 200.0` - Vertical space per layer
- `NODE_SPACING = 50.0` - Gap between nodes in same layer

## Notes
- Shares `GraphNode` struct with force-directed view for state consistency
- Handles cycles by BFS ordering (later nodes get higher ranks)
- Good for threads with clear hierarchical structure
