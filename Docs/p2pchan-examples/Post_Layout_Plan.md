# Post Layout Implementation Plan

## Overview
This document outlines the plan for implementing multiple layout options for post arrangement in thread views. The system will support switching between different layout modes, with collision detection and zoom/pan capabilities.

## Layout Types
- **Hierarchical** (Default): Tree-based layout showing reply relationships
- **Linear**: Traditional vertical chronological layout
- **Spatial**: Free-form 2D layout with manual positioning

## Implementation Phases

### Phase 1: Core Infrastructure

#### Layout Management
```rust
enum LayoutType {
    Hierarchical,  // Default tree-based layout
    Linear,        // Traditional vertical layout
    Spatial,       // Free-form 2D layout
}
```

#### Core Features
1. Layout manager implementation
2. Zoom/pan controls
   - Mouse wheel for zoom
   - Middle mouse or Alt+drag for panning
3. Coordinate system transformations
4. Basic collision detection infrastructure

#### Required ThreadView Fields
```rust
struct ThreadView {
    // Existing fields...
    layout_type: LayoutType,
    zoom_level: f32,
    view_offset: Vec2,
    collision_system: CollisionGrid, // or QuadTree
}
```

### Phase 2: Hierarchical Layout

#### Features
1. Post relationship tree construction
2. Tree layout algorithm
   - Root post positioning
   - Child post branching
   - Force-directed layout optimization
   - Collision avoidance
3. Visual connection lines between posts
4. Transition animations

#### Technical Requirements
- Enhanced SQL queries for parent/child relationships
- Efficient tree traversal algorithms
- Connection line rendering system

### Phase 3: Linear Layout

#### Features
1. Vertical post arrangement
2. Reply indentation system
3. Layout transitions
4. Scroll position maintenance

#### Technical Requirements
- Efficient vertical spacing calculations
- Indentation level tracking
- Smooth animation system

### Phase 4: Spatial Layout

#### Features
1. Drag-and-drop positioning
2. Real-time collision detection
3. Optional grid snapping
4. Position persistence
5. Auto-arrangement option

#### Technical Requirements
- Drag-and-drop system
- Enhanced collision detection
- Database schema updates for position storage
- Grid system implementation

### Phase 5: UI and Polish

#### Features
1. Layout switcher UI
2. Layout-specific controls
3. Visual mode indicators
4. Zoom controls
   - Zoom level display
   - Reset button
5. Thread mini-map
6. Inter-layout transitions

#### Technical Requirements
- UI component integration
- State management for layout options
- Mini-map rendering system

## Technical Considerations

### Collision Detection
- Quadtree or spatial hash implementation
- Soft collision resolution
- Window size consideration in calculations

### Coordinate Systems
- World coordinates (absolute positions)
- Screen coordinates (viewport-relative)
- Coordinate transformation utilities


### Performance Optimizations
1. Efficient layout calculations
2. Viewport culling
3. Animation optimization
4. Batch position updates

## Future Considerations
- Custom layout creation
- Layout templates
- Layout sharing between users
- Advanced animation options
- Keyboard shortcuts for layout manipulation

## Dependencies
- egui support for custom layouts
- SQLite schema modifications
- Additional crates for specific algorithms:
  - `petgraph` for graph layouts
  - `nalgebra` for coordinate transformations
  - Custom collision detection implementation 