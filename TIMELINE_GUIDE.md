# Chronological Layout - Quick Start Guide

## What's New?
A new "Timeline" view mode for threads that arranges posts chronologically with clean orthogonal edges, similar to the diagram you provided.

## How to Use

### Accessing Timeline View
1. Open any thread in OrbWeaver
2. Look for the view selector buttons at the top: **[Posts] [Graph] [Timeline]**
3. Click **"Timeline"** to switch to the chronological layout

### Controls
- **Pan/Scroll**: Right-click and drag to move around the canvas
- **Click Post IDs**: Click on `#post-id` to select/highlight a post
- **Reply**: Use the "↩ Reply" button within each post card
- **Quote**: Use the "❝ Quote" button to quote post content

### Layout Behavior

**Vertical (Y-axis)**: Time progression
- Top of screen = Older posts
- Bottom of screen = Newer posts
- Posts grouped into 5-minute bins

**Horizontal (X-axis)**: Sequential within time bin
- Left to right order within each time slice
- First post in bin appears leftmost

**Edges/Arrows**: Orthogonal (right-angled) paths
- Clean Manhattan-style routing
- Straight down when child is below parent
- S-curves for horizontal offsets
- Route-around logic for complex relationships

### Visual Cues
- **Selected post**: Golden border (#FAD06C)
- **Reply target**: Orange border (#FFBE5C)
- **Hovered post**: Lighter background
- **Edge colors**:
  - Orange: Reply target edges
  - Light blue: Selected post edges
  - Blue-grey: Normal edges

## Comparison with Other Views

| Feature | List | Graph | Timeline |
|---------|------|-------|----------|
| Layout | Vertical list | Force-directed | Time-binned rows |
| Edges | None shown | Diagonal | Orthogonal |
| Time Info | Per-post | Physics-based | Vertical axis |
| Best For | Reading | Exploration | Understanding flow |

## Tips
- Use Timeline view to understand conversation chronology
- Great for seeing how discussions evolved over time
- Easier to trace reply chains than in Graph view
- Panning lets you navigate long threads efficiently

## Configuration
Currently uses fixed settings:
- Time bins: 5 minutes
- Card width: 320px
- Horizontal spacing: 30px
- Vertical bin spacing: 60px

(Future versions may make these configurable via UI)

## Known Limitations
- No zoom support yet (coming soon)
- Very dense threads may need more horizontal space
- Edge bundling not yet implemented for high-degree nodes

## Troubleshooting

**Problem**: Layout looks cluttered
**Solution**: Try reducing the time window or filtering posts

**Problem**: Can't see all posts
**Solution**: Right-click drag to pan around the canvas

**Problem**: Edges overlap cards
**Solution**: This is a known issue with very complex reply structures - we're working on better routing

## Next Steps
Try it out with your existing threads! The Timeline view is particularly useful for:
- Long discussions that span hours/days
- Understanding branching conversations  
- Seeing when activity spikes occurred
- Tracing how ideas evolved chronologically
