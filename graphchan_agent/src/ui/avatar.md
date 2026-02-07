# ui/avatar.rs

## Purpose
Avatar loading and animation system for the agent UI. Supports static images (PNG, JPG) and animated GIFs with frame-accurate timing.

## Components

### `Avatar`
- **Does**: Single avatar (static or animated)
- **Fields**: frames (Vec<AvatarFrame>), current_frame, last_frame_time, is_animated

### `AvatarFrame`
- **Does**: Single frame with texture and duration
- **Fields**: texture (TextureHandle), duration (Duration)

### `AvatarSet`
- **Does**: Container for all avatar states
- **Fields**: idle, thinking, active (all Option<Avatar>)

## Avatar Methods

### `Avatar::load`
- **Does**: Load avatar from file path
- **Inputs**: egui context, path string
- **Supports**: .png, .jpg, .jpeg, .gif
- **Returns**: Result<Avatar, String>

### `Avatar::load_static`
- **Does**: Load PNG/JPG as single-frame avatar
- **Duration**: Zero (static)

### `Avatar::load_animated_gif`
- **Does**: Load GIF with all frames and timing
- **Uses**: image::codecs::gif::GifDecoder
- **Default frame delay**: 100ms if not specified

### `Avatar::update`
- **Does**: Advance animation frame if duration elapsed
- **Called**: Every render frame

### `Avatar::current_texture`
- **Does**: Get current frame's texture for rendering
- **Returns**: &TextureHandle

### `Avatar::reset`
- **Does**: Reset animation to first frame

## AvatarSet Methods

### `AvatarSet::load`
- **Does**: Load all avatars from config paths
- **Inputs**: context, idle_path, thinking_path, active_path
- **Logs warnings**: If any path fails to load

### `AvatarSet::get_for_state`
- **Does**: Get appropriate avatar for visual state
- **Mapping**:
  - Idle, Paused → idle
  - Thinking, Reading, Confused → thinking (or idle fallback)
  - Writing, Happy → active (or idle fallback)

### `AvatarSet::has_avatars`
- **Does**: Check if any avatars loaded
- **Returns**: bool

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ui/sprite.rs` | `AvatarSet`, `get_for_state`, `update` | Method changes |
| `ui/app.rs` | `AvatarSet::load`, `has_avatars` | Return types |

## State-to-Avatar Mapping

| AgentVisualState | Avatar Used |
|------------------|-------------|
| Idle | idle |
| Paused | idle |
| Thinking | thinking → idle |
| Reading | thinking → idle |
| Confused | thinking → idle |
| Writing | active → idle |
| Happy | active → idle |

## Notes
- GIF frames extracted using image crate's AnimationDecoder
- Textures created via egui's load_texture
- Frame timing from GIF delay metadata
- Fallback to idle if specific state avatar not configured
- Animation requires ctx.request_repaint() each frame
