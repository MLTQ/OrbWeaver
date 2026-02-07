# config.rs

## Purpose
Configuration management for the agent. Defines all configurable parameters including LLM settings, Graphchan connection, behavior tuning, ComfyUI integration, and character card fields. Supports TOML file persistence and environment variable fallback.

## Components

### `AgentConfig`
- **Does**: Main configuration struct with all agent settings
- **Fields**: graphchan_api_url, llm_api_url, llm_model, llm_api_key, username, system_prompt, poll_interval_secs, respond_to, enable_self_reflection, reflection_interval_hours, guiding_principles, database_path, enable_image_generation, comfyui, character fields, avatar paths
- **Persistence**: `agent_config.toml` next to executable

### `AgentConfig::load`
- **Does**: Loads config from TOML file, falls back to env vars
- **Returns**: `AgentConfig` (never fails, uses defaults)

### `AgentConfig::save`
- **Does**: Persists current config to TOML file
- **Returns**: `Result<()>`

### `AgentConfig::from_env`
- **Does**: Creates config from environment variables (legacy)
- **Env vars**: GRAPHCHAN_API_URL, LLM_API_URL, LLM_MODEL, LLM_API_KEY, AGENT_CHECK_INTERVAL, AGENT_NAME

### `RespondTo`
- **Does**: Response strategy configuration
- **Fields**: response_type ("selective", "all", "mentions"), decision_model

### `ComfyUIConfig`
- **Does**: ComfyUI image generation settings
- **Fields**: api_url, workflow_type, model_name, vae_name, width, height, steps, cfg_scale, sampler, scheduler, negative_prompt

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `main.rs` | `AgentConfig::load()` | Load signature |
| `agent/mod.rs` | All config fields | Field removal/rename |
| `ui/settings.rs` | Mutable access to all fields | Field changes |

## Config File Format

```toml
graphchan_api_url = "http://localhost:8080"
llm_api_url = "http://localhost:11434"
llm_model = "llama3.2"
username = "Agent Name"
poll_interval_secs = 60
enable_self_reflection = true
guiding_principles = ["helpful", "curious"]

[comfyui]
api_url = "http://127.0.0.1:8188"
model_name = "v1-5-pruned-emaonly.safetensors"
```

## Notes
- Config path is relative to executable for portability
- Environment variables override defaults but not file values
- Character card fields support TavernAI V2 imports
- Avatar paths for animated GIF support in UI
