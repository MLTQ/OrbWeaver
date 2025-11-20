# Main Window Design Document

## Overview
The main window serves as the primary interface for GraphChan, implemented using egui/eframe with glow rendering.

## Layout Structure
+------------------------------------------+
| File Edit View Help | <- Menu Bar
+------------------------------------------+
| |
| Logo Area | <- Header Section
| Welcome Message |
| Update News |
| |
+------------------------------------------+
| |
| |
| Thread Grid Area | <- Content Area
| (Thumbnails + Titles) |
| |
| |
+------------------------------------------+

## Components

### 1. Menu Bar
- Fixed position at top
- Standard dropdown menus:
  - File: (TBD)
  - Edit: (TBD)
  - View: (TBD)
  - Import: Import threads from external sources
  - Tools: Database management tools
  - Help: (TBD)

### 2. Header Section
- Height: ~20% of window
- Components:
  - Logo: Centered, scalable
  - Welcome Message: Below logo
  - Update News: Collapsible section
- Styling:
  - Clear visual separation from thread grid
  - Consistent padding/margins
  - Optional: Subtle background difference

### 3. Thread Grid Area
- Dynamic height (fills remaining space)
- Grid layout:
  - Responsive columns based on window width (200-250px per card)
  - Square thumbnails (120x120px display size)
  - 16px spacing between grid items
- Thread Preview Cards:
  - Vertical layout with consistent spacing:
    - OP ID and post count at top
    - Square thumbnail (or placeholder) centered
    - Title centered below thumbnail
    - Preview text (up to 100 chars, 2 lines)
    - Last update timestamp at bottom
  - Card styling:
    - Light shadow effect
    - Rounded corners (8px radius)
    - Consistent padding
- Empty State:
  - Clear "No Threads" message
  - Quick action: "Create First Thread" button
- Performance optimizations:
  - Thumbnails stored at 240x240 for 2x display quality
  - Efficient SQL query with CTE for first posts
  - Limited to 100 most recent threads
  - Deduplication of thread entries

## Technical Implementation Notes

### State Management
pub struct MainWindow {
// Window state
header_collapsed: bool,
// Content state
threads: Vec<ThreadPreview>,
// Layout state
grid_columns: u32,
}
pub struct ThreadPreview {
id: ThreadId,
title: String,
thumbnail: Option<TextureHandle>,
post_count: u32,
last_update: DateTime<Utc>,
}
### Key Functions
impl MainWindow {
// Initialize window and load initial state
pub fn new(cc: &eframe::CreationContext<'>) -> Self;
// Main render function
pub fn update(&mut self, ctx: &egui::Context);
// Component render functions
fn render_menu_bar(&self, ui: &mut egui::Ui);
fn render_header(&mut self, ui: &mut egui::Ui);
fn render_thread_grid(&mut self, ui: &mut egui::Ui);
// Thread loading and management
async fn load_thread_previews(&mut self, ctx: &Context, db: &Database) -> Result<(), anyhow::Error>;
// Layout calculations
fn calculate_grid_layout(&mut self, available_width: f32);
}
## Interaction Behaviors

### Menu Bar
- Standard dropdown behavior
- Keyboard shortcuts (to be defined)

### Header Section
- Collapsible/expandable
- Logo clickable to return to home state
- Update news scrollable if content overflows

### Thread Grid
- Thumbnails clickable to open thread
- Responsive resizing with window
- Smooth loading of thumbnails
- Optional: Infinite scroll or pagination

## Visual Style Guidelines
- Use egui's built-in themes as base
- Consistent spacing:
  - 16px between grid items
  - 8px after thumbnail
  - 4px between text elements
- Card dimensions:
  - Min width: 200px
  - Max width: 250px
  - Thumbnail: 120x120px display size
- Typography:
  - Thread titles: 14px, bold
  - Preview text: 12px, regular
  - Metadata: Small, system font
- Visual hierarchy:
  - Thumbnail prominent and centered
  - Title emphasized with bold weight
  - Preview text in lighter color
  - Metadata (timestamps, counts) smallest

## Performance Considerations
- Efficient thread loading:
  - Uses CTE for first post lookup
  - Limits to 100 most recent threads
  - Prevents duplicate thread entries
- Thumbnail optimization:
  - Source images resized to 240x240
  - Display size 120x120 for crisp rendering
  - Triangle filter for quality downscaling
- Layout efficiency:
  - Responsive grid calculation
  - Minimal reflows
  - Cached texture handles

## Future Enhancements
- Thread sorting options
- Grid view customization
- Advanced filtering
- Search functionality
- Customizable layouts
- Infinite scroll pagination
- Thumbnail caching system
- Dynamic thumbnail loading

### Menu Functions
- **Import**
  - From 4chan: Import threads with progress tracking
  - From Reddit: (Planned)
  - From X: (Planned)
- **Tools**
  - Clean Database: Safely removes all content
    - Deletes all threads, posts, and images
    - Removes downloaded files
    - Shows confirmation dialog
    - Auto-refreshes thread grid after cleanup

### Thread Grid Behavior
- Automatic updates after:
  - Thread import completion
  - Database cleanup
  - (Future) Thread creation
- Dynamic layout adjustment
- Responsive to window resizing