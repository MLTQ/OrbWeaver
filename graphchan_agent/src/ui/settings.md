# ui/settings.rs

## Purpose
Settings panel for configuring the agent. Provides UI for all AgentConfig fields including connection settings, LLM config, behavior tuning, and image generation options.

## Components

### `SettingsPanel`
- **Does**: Modal settings window
- **Fields**: config (AgentConfig), show (bool)

### `SettingsPanel::new`
- **Does**: Create panel with initial config
- **Inputs**: AgentConfig

### `SettingsPanel::render`
- **Does**: Render settings window
- **Returns**: Option<AgentConfig> (Some if saved)

## Settings Sections

### Graphchan Connection
- API URL text field

### LLM Configuration
- API URL (e.g., http://localhost:11434 for Ollama)
- Model name (e.g., llama3.2, qwen2.5)
- API Key (optional, for OpenAI/Claude)

### Agent Identity
- Username (displayed in posts)

### Behavior
- Poll interval (seconds, 10-600)
- Max posts per hour (rate limiting)
- Response strategy (selective/all/mentions)

### Self-Reflection & Evolution
- Enable checkbox
- Reflection interval (hours)
- Guiding principles (multiline text, one per line)

### Memory & Database
- Database path
- Max important posts

### Image Generation (ComfyUI)
- Enable checkbox
- ComfyUI URL
- Workflow type (SD, SDXL, Flux)
- Model name

### System Prompt
- Multiline text area for custom prompt

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ui/app.rs` | `SettingsPanel::new`, `render` | Return type |
| `config.rs` | All config fields exposed | Field changes |

## Notes
- Save & Apply button triggers agent reload
- Config file path shown: agent_config.toml
- Guiding principles parsed from newline-separated text
- DragValue widgets for numeric fields with ranges
- ComboBox for response strategy selection
