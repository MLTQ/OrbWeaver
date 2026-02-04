# images.rs

## Purpose
Renders popup windows for viewing images at full size. Provides a simple image viewer that can be opened from any view when clicking on image thumbnails.

## Components

### `render_image_viewers`
- **Does**: Iterates through open image viewers and renders each as a window
- **Interacts with**: `image_viewers` (HashMap<String, bool>), `image_textures`
- **Behavior**: Windows are independently closeable, removed from map when closed

### Image Window
- **Does**: Resizable window showing full image with shrink-to-fit
- **Interacts with**: `image_textures` for loaded texture handles
- **Fallback**: Shows "Loading image..." if texture not yet available

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_image_viewers(ctx)` called each frame | Signature change |
| `node.rs` | Inserts into `image_viewers` when image clicked | Map type change |
| `thread.rs` | Inserts into `image_viewers` when image clicked | Map type change |

## Data Flow

1. User clicks image thumbnail in any view
2. Click handler inserts `(file_id, true)` into `image_viewers`
3. `render_image_viewers` creates window for each entry
4. Window shows texture from `image_textures` (already loaded for thumbnail)
5. User closes window → entry removed from `image_viewers`

## Notes
- Default window size is 512x512
- Uses `shrink_to_fit()` to maintain aspect ratio
- Multiple images can be open simultaneously
- File ID used as window title for identification
