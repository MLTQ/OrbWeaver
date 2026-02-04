# settings.rs

## Purpose
Renders the application settings page with configuration options for API connection and theme customization. Allows users to connect to different backend instances and personalize the visual appearance.

## Components

### `render_settings`
- **Does**: Main settings page renderer with grouped configuration sections
- **Interacts with**: `GraphchanApp` fields for URL, theme colors

### API Configuration Section
- **Does**: Backend URL input with Apply/Reset buttons
- **Interacts with**: `api.set_base_url`, `spawn_load_threads`
- **Behavior**: Changing URL reconnects to different instance, reloads threads

### Theme Customization Section
- **Does**: Color picker for primary color with generated scheme preview
- **Interacts with**: `primary_color`, `theme_dirty`, `api.set_theme_color`
- **Features**: Live preview of generated color scheme (BG, Card, Primary, Highlight)

### Generated Color Scheme
- **Does**: Shows color swatches derived from primary color
- **Interacts with**: `color_theme::ColorScheme::from_primary`
- **Display**: Background (quaternary), Card (tertiary), Primary, Highlight

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `mod.rs` (app) | `render_settings(ui)` for `ViewState::Settings` | Signature change |
| `api.rs` | `set_base_url(url)` returns `Result` | Return type change |
| `color_theme.rs` | `ColorScheme::from_primary(color)` generates scheme | Function signature |

## Layout

```
⚙ Settings

┌─ API Configuration ──────────────────────────┐
│ Backend URL: [http://127.0.0.1:8080      ]   │
│ [Apply Changes] [Reset to Default]           │
│ ⚠ Changing the backend URL will reconnect... │
└──────────────────────────────────────────────┘

┌─ Theme Customization ────────────────────────┐
│ Primary Color: [■]                           │
│                                              │
│ Generated Color Scheme:                      │
│ [BG] [Card] [Primary] [Highlight]            │
└──────────────────────────────────────────────┘
```

## Notes
- Theme changes saved to backend for persistence across sessions
- `theme_dirty` flag triggers style reapplication on next frame
- Color scheme uses HSL manipulation to derive related colors from primary
