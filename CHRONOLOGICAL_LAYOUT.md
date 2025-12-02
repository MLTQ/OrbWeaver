# Chronological Layout Implementation

## Overview
Added a new "Timeline" view mode for thread visualization that arranges posts chronologically in time-binned rows with orthogonal (Manhattan-style) edge routing.

## Changes Made

### 1. New Display Mode (`src/app/state.rs`)
- Added `Chronological` variant to `ThreadDisplayMode` enum
- Added `last_layout_mode: Option<ThreadDisplayMode>` field to `ThreadState` to track mode switches

### 2. New Module (`src/app/ui/chronological.rs`)
**Key Features:**
- **Time Binning**: Posts grouped into N-minute intervals (default: 5 minutes)
- **Chronological Sorting**: Oldest posts at top, newest at bottom (like your diagram)
- **Horizontal Positioning**: Posts arranged left-to-right within each time bin
- **Orthogonal Edge Routing**: Clean Manhattan-style paths with right angles
  - Straight down for simple child-below-parent cases
  - S-curve for horizontally offset children
  - Route-around logic for complex cases (child above/beside parent)

**Layout Parameters:**
Parameters like `BIN_MINUTES` are now configurable via the UI stored in `ThreadState`.
Default values:
```rust
const DEFAULT_BIN_MINUTES: i64 = 5;
const CARD_WIDTH: f32 = 320.0;
const CARD_HORIZONTAL_SPACING: f32 = 30.0;
const BIN_VERTICAL_SPACING: f32 = 60.0;
```

### 3. Edge Routing Logic
Three routing strategies based on relative positions:

**Case 1: Child Below Parent (Simple)**
- Horizontally aligned (< 50px offset): Straight vertical line
- Horizontally offset: S-curve with horizontal midpoint transition

**Case 2: Child Above/Beside Parent (Complex)**
- Routes up → across → down using waypoints
- Maintains clearance from card boundaries
- Creates clean orthogonal paths

### 4. Integration (`src/app/ui/thread.rs`, `src/app/ui/mod.rs`)
- Added "Timeline" button to view selector
- Wired chronological rendering into view pipeline
- Added mode-switch detection to clear/rebuild layouts when changing views
- Made `image_preview` and `ImagePreview` public in graph.rs for reuse

## Architecture

### Data Flow
```
PostView[] 
  → parse timestamps
  → sort chronologically
  → group into time bins
  → assign Y coords (bin index * spacing)
  → assign X coords (position within bin)
  → HashMap<PostId, GraphNode>
```

### Edge Rendering
```
parent_rect, child_rect
  → determine anchor points (top/bottom centers)
  → choose routing strategy based on relative position
  → generate waypoint path
  → draw line segments + arrowhead
```

## Usage
1. Open any thread
2. Click "Timeline" button in view selector
3. Pan with right-click drag
4. Zoom with scroll wheel
5. Adjust time bin size with slider
6. Posts arranged in time-descending order with clean orthogonal connections

## Future Enhancements
- [x] Configurable time bin size (UI setting)
- [x] Zoom support
- [ ] Smart edge bundling for dense graphs
- [ ] Horizontal timeline scrubber
- [ ] Collapsible time bins
- [ ] Minimap overview

## Technical Notes
- Uses `chrono` for timestamp parsing
- Reuses image loading/rendering from graph module
- Layout is cached in `ThreadState.graph_nodes` until mode switch
- All coordinates are absolute (pixels) not normalized (0-1)
- Background uses same dot grid pattern as force-directed graph

## Testing Recommendations
1. Test with threads of varying post counts (1, 5, 20, 100+)
2. Verify edge routing for various parent-child configurations
3. Check performance with large threads
4. Test mode switching between List/Graph/Chronological
5. Verify panning and interaction feel natural
