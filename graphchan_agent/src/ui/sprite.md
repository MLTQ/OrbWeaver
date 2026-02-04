# ui/sprite.rs

## Purpose
Renders the agent's visual representation - either a custom avatar image or an emoji fallback based on the current visual state.

## Components

### `render_agent_sprite`
- **Does**: Main sprite rendering function
- **Inputs**: ui, &AgentVisualState, Option<&mut AvatarSet>
- **Flow**:
  1. If avatars available and state has avatar → render image
  2. Otherwise → render emoji fallback

### `render_agent_emoji`
- **Does**: Fallback emoji rendering
- **Inputs**: ui, &AgentVisualState
- **Size**: 48pt emoji

## State-to-Emoji Mapping

| State | Emoji | Color |
|-------|-------|-------|
| Idle | 😴 | Gray |
| Reading | 📖 | Light Blue |
| Thinking | 🤔 | Yellow |
| Writing | ✍️ | Light Green |
| Happy | 😊 | Green |
| Confused | 😕 | Orange |
| Paused | ⏸️ | Light Red |

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ui/app.rs` | `render_agent_sprite` | Signature change |
| `agent/mod.rs` | `AgentVisualState` enum | Variant changes |
| `ui/avatar.rs` | `AvatarSet::get_for_state`, `Avatar::update` | Method changes |

## Rendering Flow

```
render_agent_sprite()
        │
        ├── avatars.is_some()?
        │         │
        │    Yes  │  No
        │         │
        ▼         ▼
get_for_state()  render_agent_emoji()
        │
   avatar exists?
        │
   Yes  │  No
        │
        ▼
  update()
  current_texture()
  egui::Image (64x64)
  request_repaint() if animated
```

## Notes
- Avatar size fixed at 64x64 pixels
- Emoji rendered as heading for large size
- Animated avatars trigger repaint for smooth animation
- Falls back to emoji if avatar loading fails
