# main.rs

## Purpose
Entry point for the Graphchan Agent application. Initializes logging, loads configuration, creates the agent with its reasoning engine, spawns the agent loop in a background thread, and launches the egui UI.

## Components

### `main`
- **Does**: Application entry point
- **Flow**:
  1. Initialize tracing/logging
  2. Load `AgentConfig` from TOML file
  3. Create `GraphchanClient` for API communication
  4. Create event channel (flume) for agent→UI communication
  5. Create `AgentDatabase` for memory persistence
  6. Create `Agent` with all components
  7. Spawn agent loop in background thread with Tokio runtime
  8. Launch eframe/egui UI with `AgentApp`

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/mod.rs` | `Agent::new()`, `Agent::run_loop()` | Signature changes |
| `config.rs` | `AgentConfig::load()` | Return type changes |
| `ui/app.rs` | `AgentApp::new()` | Constructor changes |

## Architecture

```
┌─────────────────────────────────────────────────┐
│                    main()                        │
└─────────────────────────────────────────────────┘
                      │
         ┌────────────┼────────────┐
         ▼            ▼            ▼
    ┌─────────┐  ┌─────────┐  ┌─────────┐
    │  Agent  │  │   UI    │  │ Database│
    │  Loop   │  │ (egui)  │  │ (SQLite)│
    └────┬────┘  └────┬────┘  └─────────┘
         │            │
         └────────────┘
           flume channel
```

## Notes
- Agent runs in separate thread with its own Tokio runtime
- UI runs on main thread (required by eframe/egui)
- Communication via flume unbounded channel (AgentEvent)
- Database uses WAL mode for concurrent read access from UI
