# mod.rs (ui module)

## Purpose
Module declaration file for the UI subsystem. Re-exports all view modules and establishes the UI component hierarchy.

## Submodules

### Thread Views
- **thread** - List view and thread utilities
- **graph** - Force-directed physics layout
- **sugiyama** - Hierarchical layered layout
- **chronological** - Timeline with time binning
- **radial** - Phylogenetic tree layout

### Shared Components
- **node** - Shared post card rendering for graph views
- **input** - Keyboard navigation handlers
- **reaction_colors** - Reaction-based color calculation

### Main Views
- **catalog** - Thread listing grid
- **images** - Image viewer popups
- **dialogs** - Create thread, import dialogs

### Social Features
- **friends** - Peer management UI
- **conversations** - Direct messaging UI
- **blocking** - User blocking management

### Settings & Management
- **settings** - App settings panel
- **topics** - Topic subscription management
- **search** - Search functionality
- **drawer** - Identity sidebar

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `app/mod.rs` | All submodules publicly exported | Module removal |
| External | `use crate::app::ui::*` imports | Re-export changes |

## Notes
- All modules are `pub mod` for external access
- No logic in this file, purely module organization
- Each view module is self-contained with its own state handling
