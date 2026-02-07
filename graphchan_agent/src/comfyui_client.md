# comfyui_client.rs

## Purpose
Alternative ComfyUI client that builds minimal workflows programmatically. Creates SD/SDXL workflows from configuration rather than importing external workflow files.

## Components

### `ComfyUIClient`
- **Does**: ComfyUI client with built-in workflow generation
- **Fields**: api_url, config (ComfyUIConfig), client

### `PromptResponse`
- **Fields**: prompt_id

### `HistoryResponse`
- **Fields**: prompts (HashMap<String, PromptHistory>)

### `PromptHistory`
- **Fields**: outputs (HashMap<String, NodeOutput>)

### `NodeOutput`
- **Fields**: images (Option<Vec<ImageOutput>>)

### `ImageOutput`
- **Fields**: filename, subfolder, output_type

## Key Methods

### `generate_image`
- **Does**: End-to-end image generation
- **Inputs**: prompt, negative_prompt
- **Returns**: Result<Vec<u8>> (image bytes)
- **Flow**: Build workflow → Queue → Poll → Download

### `build_minimal_workflow`
- **Does**: Programmatically create ComfyUI workflow JSON
- **Nodes created**:
  - Node 3: KSampler
  - Node 4: CheckpointLoaderSimple
  - Node 5: EmptyLatentImage
  - Node 6: CLIPTextEncode (positive)
  - Node 7: CLIPTextEncode (negative)
  - Node 8: VAEDecode
  - Node 9: SaveImage
  - Node 10: VAELoader (optional)

### `poll_until_complete`
- **Does**: Wait for generation with timeout
- **Timeout**: 60 attempts × 2 seconds = 2 minutes
- **Returns**: Result<ImageOutput>

### `download_image`
- **Does**: Fetch image bytes from ComfyUI
- **Returns**: Result<Vec<u8>>

### `prompt_style`
- **Does**: Get prompt style for workflow type
- **Returns**: "tags" (SD/SDXL) or "natural" (Flux)

### `workflow_type`
- **Returns**: Reference to configured WorkflowType

### `negative_prompt`
- **Returns**: Reference to configured negative prompt

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| (image generation features) | `generate_image` | Return type |

## Workflow Structure

```json
{
  "3": { "class_type": "KSampler", ... },
  "4": { "class_type": "CheckpointLoaderSimple", ... },
  "5": { "class_type": "EmptyLatentImage", ... },
  "6": { "class_type": "CLIPTextEncode", ... },  // positive
  "7": { "class_type": "CLIPTextEncode", ... },  // negative
  "8": { "class_type": "VAEDecode", ... },
  "9": { "class_type": "SaveImage", ... },
  "10": { "class_type": "VAELoader", ... }       // optional
}
```

## Notes
- Uses random seed for each generation
- Supports optional dedicated VAE loader
- Config-driven: model, steps, CFG, sampler, scheduler
- 5-minute HTTP timeout for long generations
