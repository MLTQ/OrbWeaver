# comfy_client.rs

## Purpose
ComfyUI API client for executing workflows and retrieving generated images. Handles prompt queuing, status polling, and image download from ComfyUI's REST API.

## Components

### `ComfyUIClient`
- **Does**: HTTP client for ComfyUI API
- **Fields**: api_url, client (reqwest::Client)

### `QueuePromptRequest`
- **Does**: Request body for queuing workflow
- **Fields**: prompt (workflow JSON), client_id

### `QueuePromptResponse`
- **Does**: Response with prompt ID
- **Fields**: prompt_id

### `HistoryEntry`
- **Does**: Execution history entry
- **Fields**: outputs (HashMap<String, OutputNode>), status

### `OutputNode`
- **Does**: Node output containing images
- **Fields**: images (Option<Vec<ImageInfo>>)

### `ImageInfo`
- **Does**: Generated image metadata
- **Fields**: filename, subfolder, image_type

### `StatusInfo`
- **Does**: Execution status
- **Fields**: status_str, completed

## Key Methods

### `queue_prompt`
- **Does**: Submit workflow for execution
- **Endpoint**: POST /prompt
- **Returns**: Result<String> (prompt_id)

### `get_history`
- **Does**: Check execution status
- **Endpoint**: GET /history/{prompt_id}
- **Returns**: Result<Option<HistoryEntry>>

### `wait_for_completion`
- **Does**: Poll until workflow completes
- **Timeout**: Configurable (default 300s)
- **Returns**: Result<ImageInfo>

### `download_image`
- **Does**: Download generated image file
- **Endpoint**: GET /view?filename=...&subfolder=...&type=...
- **Returns**: Result<PathBuf>

### `test_connection`
- **Does**: Verify ComfyUI is reachable
- **Endpoint**: GET /history
- **Returns**: Result<()>

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `agent/image_gen.rs` | All methods | Signature changes |
| `ui/comfy_settings.rs` | `test_connection` | Return type |

## Workflow Execution Flow

```
1. queue_prompt(workflow_json)
       │
       ▼
2. Poll get_history(prompt_id)
       │
       ▼
3. Check status.completed == true
       │
       ▼
4. Extract ImageInfo from outputs
       │
       ▼
5. download_image(image_info)
```

## Notes
- Polls every 1 second until completion
- Saves downloaded images to working directory
- Compatible with standard ComfyUI API
