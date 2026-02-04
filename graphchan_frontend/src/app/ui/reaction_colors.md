# reaction_colors.rs

## Purpose
Calculates post border/stroke colors based on emoji reactions using an additive RGB model. Each reaction type contributes specific color values, creating a visual heat map of community sentiment on posts.

## Components

### `calculate_post_color`
- **Does**: Computes final RGB color from a post's reaction counts
- **Interacts with**: `node.rs` for post border rendering
- **Algorithm**: Additive RGB - each reaction adds its color delta multiplied by count

### `emoji_to_rgb_delta`
- **Does**: Maps each emoji to RGB delta values (how much to add per reaction)
- **Interacts with**: `calculate_post_color`
- **Returns**: `(r_delta, g_delta, b_delta)` tuple

### `calculate_max_score_from_responses`
- **Does**: Placeholder for normalization (returns 1.0, not used in additive model)
- **Interacts with**: Kept for API compatibility
- **Rationale**: Additive model doesn't need normalization

## Emoji Color Mappings

| Emoji | Color Target | RGB Delta | ~10 reactions |
|-------|--------------|-----------|---------------|
| 👍 | Yellow/Green | (15, 15, 0) | Bright yellow |
| ❤️ | Red/Pink | (20, 8, 10) | Vibrant pink |
| 😂 | Yellow/Orange | (18, 14, 0) | Golden |
| 🎉 | Magenta/Purple | (16, 8, 16) | Party purple |
| 🔥 | Orange/Red | (20, 10, 0) | Bright orange |
| ✨ | White | (18, 18, 18) | Brilliant white |
| 👀 | Cyan | (0, 15, 18) | Attention cyan |
| 🤔 | Purple | (12, 6, 15) | Thoughtful purple |
| 👎 | Dark Red | (10, 0, 0) | Deep red |
| 😢 | Blue | (0, 6, 12) | Melancholy blue |
| 😡 | Red | (15, 0, 0) | Angry red |
| (other) | Gray | (8, 8, 8) | Subtle gray |

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `node.rs` | `calculate_post_color(reactions, max_score)` returns `Color32` | Return type |
| `node.rs` | `calculate_max_score_from_responses(all_reactions)` returns `f32` | Return type |

## Algorithm

```
color = (0, 0, 0)  // Start black
for each (emoji, count) in reactions:
    delta = emoji_to_rgb_delta(emoji)
    color += delta * count
color = clamp(color, 0, 255)
if color == (0,0,0): return default_gray
return color
```

## Notes
- Additive model means more reactions = brighter colors
- ~10 reactions of one type reaches target saturation
- Mixed reactions blend colors (👍 + ❤️ = orange-ish)
- Default color (60, 65, 90) for posts with no reactions
- Values tuned so popular posts "glow" with sentiment color
