# agent/image_gen.rs

## Purpose
Image generation handler for the agent. Interfaces with ComfyUI to generate images based on prompts, using imported workflows with controllable nodes.

## Components

### `ImageGenerator`
- **Does**: Manages ComfyUI workflow execution
- **Fields**: client (ComfyUIClient), workflow (Option<ComfyWorkflow>)

### `ImageGenerator::new`
- **Does**: Create generator with client and optional workflow
- **Inputs**: comfy_api_url, Option<ComfyWorkflow>

### `ImageGenerator::is_available`
- **Does**: Check if image generation is configured
- **Returns**: bool (workflow.is_some())

### `ImageGenerator::generate_image`
- **Does**: Generate image from prompt using workflow
- **Flow**:
  1. Find text inputs in controllable nodes
  2. Populate prompt and seed values
  3. Prepare workflow for execution
  4. Queue prompt with ComfyUI
  5. Wait for completion
  6. Download generated image
- **Returns**: Result<PathBuf>

### `ImageGenerator::generate_prompt_from_context`
- **Does**: Create image prompt from thread context
- **Inputs**: context string, llm_client reference
- **Returns**: Result<String>
- **Note**: Currently uses simple heuristic, TODO: use LLM

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/mod.rs` | `ImageGenerator::new`, `is_available` | Constructor |
| `config.rs` | Workflow loaded from `workflow_settings` JSON | Format changes |

## Workflow Input Mapping

| Input Type | Action |
|------------|--------|
| text | Populated with user prompt |
| seed | Set to -1 (random) |
| Other | Use default values |

## Notes
- Requires ComfyWorkflow loaded from config
- Only modifies inputs where `agent_modifiable=true`
- 5-minute timeout for image generation
- Generated images saved to working directory
- LLM-based prompt generation planned but not implemented
