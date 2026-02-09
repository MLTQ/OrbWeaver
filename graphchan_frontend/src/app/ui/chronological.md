# chronological.rs

## Purpose
Renders threads as a timeline with posts grouped into configurable time bins. Posts stack horizontally within each bin, with a time axis showing when activity occurred. Optimized for understanding temporal patterns in conversations.

## Components

### `ChronoBin`
- **Does**: Groups posts that fall within the same time window
- **Interacts with**: `build_chronological_layout`

### `build_chronological_layout`
- **Does**: Assigns positions based on time bins and horizontal stacking
- **Interacts with**: `ThreadState.chrono_nodes`
- **Rationale**: Posts in same time bin stack horizontally to show concurrent activity

### `create_time_bins`
- **Does**: Groups timestamped posts into bins of configurable duration
- **Interacts with**: `build_chronological_layout`

### `render_chronological_view`
- **Does**: Main render function - draws time axis, edges, and nodes
- **Interacts with**: `GraphchanApp`, `ThreadState`, `node::render_node`

### `estimate_node_height`
- **Does**: Estimates card height based on content length and attachments
- **Interacts with**: Layout calculation for bin spacing

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `input.rs` | `chrono_nodes` map with `GraphNode.pos` | Map type change |
| `state.rs` | `ThreadState.time_bin_seconds` for bin size | Field removal |
| `mod.rs` | `render_chronological_view(app, ui, state)` signature | Function signature |

## Algorithm Details

1. **Parse & Sort**: Convert timestamps to `DateTime<Utc>`, sort oldest first
2. **Bin Creation**: Group posts by `floor(timestamp / bin_seconds)`
3. **Position Assignment**:
   - Y = cumulative bin height (sum of previous bin heights)
   - X = `LEFT_MARGIN + (index_in_bin * (CARD_WIDTH + spacing))`
4. **Height Estimation**: Each bin's height = max(post heights in bin) + spacing

## Constants

- `CARD_WIDTH = 720.0` - Matches 4chan width for familiarity
- `CARD_HORIZONTAL_SPACING = 50.0` - Gap between cards in same bin
- `MIN_BIN_VERTICAL_SPACING = 200.0` - Minimum space between bins
- `LEFT_MARGIN = 120.0` - Room for time axis labels

## Notes
- Time axis shows bin start timestamps on left margin
- Bin size configurable: 1 min, 5 min, 15 min, 1 hour, 24 hours
- Edges curve between posts across time bins
- Good for analyzing discussion patterns and response times
- After rendering, cached node heights are updated with actual rendered heights to fix edge attachment accuracy (estimates oversize; actual `render_node` output is the source of truth)
