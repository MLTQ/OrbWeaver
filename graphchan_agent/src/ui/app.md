# ui/app.rs

## Purpose
Main egui application for the agent UI. Coordinates event display, chat interface, settings panels, and avatar rendering. Implements eframe::App for the application loop.

## Components

### `AgentApp`
- **Does**: Main application state
- **Fields**: events, event_rx, agent, current_state, user_input, runtime, settings_panel, character_panel, comfy_settings_panel, avatars, database, chat_history, last_chat_refresh, show_chat_panel

### `AgentApp::new`
- **Does**: Initialize UI with agent and config
- **Inputs**: event_rx (Receiver), agent (Arc<Agent>), config, database

### `AgentApp::refresh_chat_history`
- **Does**: Reload chat from database
- **Frequency**: Every 2 seconds

### `AgentApp::send_chat_message`
- **Does**: Send operator message to agent
- **Role**: "operator"

### `AgentApp::load_avatars`
- **Does**: Load avatar images from config paths
- **Called**: First frame when egui context available

## eframe::App Implementation

### `update`
- **Does**: Main render loop (called every frame)
- **Flow**:
  1. Load avatars if needed
  2. Refresh chat history periodically
  3. Poll events from agent channel
  4. Render header with sprite and controls
  5. Render event log or chat panel
  6. Render user input
  7. Render modal panels (settings, character, workflow)
  8. Handle config saves and agent reloads

## UI Layout

```
┌─────────────────────────────────────────────────┐
│ [Sprite]  Graphchan Agent    [Chat][Workflow]   │
│           Status: Thinking   [Character][Settings][Pause] │
├─────────────────────────────────────────────────┤
│                                                  │
│   Event Log / Private Chat                       │
│   (scrollable area)                              │
│                                                  │
├─────────────────────────────────────────────────┤
│ 💬 [User input field........................] [Send] │
└─────────────────────────────────────────────────┘
```

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `AgentApp::new`, eframe::App impl | Constructor |
| `agent/mod.rs` | `Agent::toggle_pause`, `Agent::reload_config` | Method removal |

## Panel Coordination

| Panel | Toggle | Save Handler |
|-------|--------|--------------|
| Settings | settings_panel.show | Save config, reload agent |
| Character | character_panel.show | Update system_prompt, save, reload |
| Workflow | comfy_settings_panel.show | Update workflow_settings, save, reload |

## Notes
- Uses Tokio runtime for async operations (pause, reload)
- Repaints every 100ms for smooth updates
- Chat/Activity toggle switches between views
- User input sends to private chat, switches to chat view
- Avatars loaded lazily on first frame (needs egui context)
