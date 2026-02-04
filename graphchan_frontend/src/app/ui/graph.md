# graph.rs

## Purpose
Renders threads as a force-directed graph using spring-embedder physics simulation. Posts are nodes connected by reply edges, with positions determined by repulsive forces between all nodes and attractive forces along edges.

## Components

### `build_initial_graph`
- **Does**: Creates initial `GraphNode` positions with OP at center, others randomly scattered
- **Interacts with**: `ThreadState.graph_nodes`
- **Rationale**: Random initial positions prevent overlapping; OP pinned as anchor

### `step_graph_layout`
- **Does**: Runs one physics simulation step (forces, velocity, position updates)
- **Interacts with**: `GraphNode.pos`, `GraphNode.vel`
- **Rationale**: Incremental simulation allows smooth animation

### `render_graph_view`
- **Does**: Main render function - runs physics, draws edges, renders nodes
- **Interacts with**: `GraphchanApp`, `ThreadState`, `node::render_node`, `input::handle_keyboard_input`

### Physics Parameters
- `repulsion`: Inverse-square repulsive force between all node pairs
- `attraction`: Spring force along edges toward desired length
- `damping`: Velocity reduction factor (0.80) for stability

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `input.rs` | `graph_nodes` map with `GraphNode.pos` field | Struct changes |
| `state.rs` | `GraphNode` has `pos`, `vel`, `pinned`, `dragging` | Field removal |
| `mod.rs` | `render_graph_view(app, ui, state)` signature | Function signature |

## Algorithm Details

1. **Initialization**: OP at (0,0) pinned, others random within [-5, 5]
2. **Each frame** (first 5 seconds):
   - Apply repulsive force between all pairs: `F = repulsion / dist²`
   - Apply attractive spring force along edges: `F = attraction * (dist - desired)`
   - Update velocities with damping
   - Update positions from velocities
3. **After settling**: Physics stops, user can drag nodes

## Interaction

- **Left-click**: Select post
- **Right-drag**: Pan viewport
- **Scroll**: Zoom in/out
- **Pin button**: Lock node position

## Notes
- Uses seeded RNG (seed=42) for reproducible initial layouts
- Edges drawn as quadratic bezier curves
- Configurable repulsion and edge length via UI sliders
