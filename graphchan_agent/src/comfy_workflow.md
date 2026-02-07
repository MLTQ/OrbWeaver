# comfy_workflow.rs

## Purpose
ComfyUI workflow parsing and manipulation. Imports workflows from PNG metadata or JSON files, auto-detects controllable nodes, and prepares workflows for execution with agent-provided inputs.

## Components

### `ComfyWorkflow`
- **Does**: Complete workflow with metadata
- **Fields**: name, workflow_json, controllable_nodes, output_node_id, preview_image_path

### `ControllableNode`
- **Does**: Node with modifiable inputs
- **Fields**: node_id, class_type, inputs (Vec<ControllableInput>)

### `ControllableInput`
- **Does**: Single controllable parameter
- **Fields**: name, input_type, default_value, agent_modifiable, description

### `InputType`
- **Does**: Type of controllable input
- **Variants**: Text, Int, Float, Bool, Seed

## Key Methods

### `ComfyWorkflow::from_png`
- **Does**: Import workflow from ComfyUI PNG metadata
- **Extracts**: "workflow" or "prompt" tEXt chunk
- **Returns**: Result<ComfyWorkflow>

### `ComfyWorkflow::from_json_file`
- **Does**: Import workflow from JSON file
- **Returns**: Result<ComfyWorkflow>

### `ComfyWorkflow::prepare_for_execution`
- **Does**: Apply agent inputs to workflow
- **Inputs**: HashMap<String, Value> of input values
- **Returns**: Result<Value> (modified workflow JSON)

### `extract_comfy_workflow_from_png`
- **Does**: Parse PNG chunks to find workflow
- **Format**: PNG tEXt chunk with "workflow" or "prompt" keyword

### `detect_controllable_nodes`
- **Does**: Auto-detect modifiable nodes
- **Detects**:
  - CLIPTextEncode (text prompts)
  - KSampler/KSamplerAdvanced (seed, steps, cfg, denoise)
  - EmptyLatentImage (width, height)

### `find_output_node`
- **Does**: Locate SaveImage or PreviewImage node
- **Returns**: Result<String> (node ID)

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/image_gen.rs` | `ComfyWorkflow`, `prepare_for_execution` | Struct/method changes |
| `ui/comfy_settings.rs` | `from_png`, `from_json_file` | Method signatures |
| `agent/mod.rs` | Serialization to JSON | Serde changes |

## Auto-Detected Nodes

| Class Type | Controllable Inputs |
|------------|---------------------|
| CLIPTextEncode | text (agent_modifiable=true) |
| KSampler | seed (true), steps, cfg, denoise (false) |
| EmptyLatentImage | width, height (false) |

## Notes
- PNG metadata uses base64-encoded JSON in tEXt chunks
- Agent can only modify inputs where `agent_modifiable=true`
- Default: only text prompts and seeds are agent-modifiable
- Workflow serialized to JSON string in config for persistence
