# ui/mod.rs

## Purpose
Module declaration for the agent's egui-based user interface. Re-exports UI components for the main application.

## Submodules

- **app** - Main application window and event loop
- **avatar** - Avatar loading and animation (PNG, JPG, GIF)
- **chat** - Event log and private chat rendering
- **sprite** - Agent sprite/emoji display
- **settings** - Settings panel for configuration
- **character** - Character card import panel
- **comfy_settings** - ComfyUI workflow panel

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `ui::app::AgentApp` | Export removal |

## Notes
- All UI components use egui immediate-mode rendering
- Panels render via egui::Window for modal dialogs
- Main app coordinates all panel visibility and state
