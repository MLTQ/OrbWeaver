# ui/comfy_settings.rs

## Purpose
ComfyUI workflow management panel. Allows importing workflows from PNG/JSON, viewing controllable nodes, configuring which inputs the agent can modify, and testing the ComfyUI connection.

## Components

### `ComfySettingsPanel`
- **Does**: Modal workflow configuration window
- **Fields**: show (bool), workflow (Option<ComfyWorkflow>), workflow_texture, import_error, test_status

### `ComfySettingsPanel::new`
- **Does**: Create empty panel

### `ComfySettingsPanel::render`
- **Does**: Render workflow panel
- **Inputs**: context, &mut AgentConfig
- **Returns**: bool (true if saved)

### `ComfySettingsPanel::import_workflow_png`
- **Does**: Import workflow from ComfyUI PNG metadata
- **Uses**: `ComfyWorkflow::from_png`

### `ComfySettingsPanel::import_workflow_json`
- **Does**: Import workflow from JSON file
- **Uses**: `ComfyWorkflow::from_json_file`

### `ComfySettingsPanel::test_workflow`
- **Does**: Test connection to ComfyUI
- **Uses**: `ComfyUIClient::test_connection`

### `ComfySettingsPanel::save_workflow_to_config`
- **Does**: Serialize workflow to config.workflow_settings
- **Format**: JSON string

### `ComfySettingsPanel::load_workflow_from_config`
- **Does**: Deserialize workflow from config on startup

## UI Layout

```
┌─────────────────────────────────────────────────┐
│ 🎨 ComfyUI Workflow                          [x]│
├─────────────────────────────────────────────────┤
│ Import Workflow                                 │
│ [📁 Import from PNG] [📄 Import from JSON]      │
├─────────────────────────────────────────────────┤
│ Current Workflow                                │
│ ┌──────┐  Name: my_workflow                     │
│ │Preview│ Output Node: 9                        │
│ │      │  Controllable Nodes: 3                 │
│ └──────┘                                        │
├─────────────────────────────────────────────────┤
│ Controllable Inputs                             │
│ ┌───────────────────────────────────────────┐   │
│ │ Node 6 (CLIPTextEncode)                   │   │
│ │ [✓] text (Text) "a photo of..."  ✓ Agent  │   │
│ └───────────────────────────────────────────┘   │
│ ┌───────────────────────────────────────────┐   │
│ │ Node 3 (KSampler)                         │   │
│ │ [✓] seed (Seed) [-1]      ✓ Agent can mod │   │
│ │ [ ] steps (Int) [20]      🔒 Fixed        │   │
│ │ [ ] cfg (Float) [7.00]    🔒 Fixed        │   │
│ └───────────────────────────────────────────┘   │
├─────────────────────────────────────────────────┤
│ [🧪 Test Workflow] [💾 Save]          [Cancel]  │
│ ✅ Connected to ComfyUI at http://127.0.0.1:8188│
└─────────────────────────────────────────────────┘
```

## Contracts

| Dependent | Expects | Breaking changes |
|-----------|---------|------------------|
| `ui/app.rs` | `render`, `load_workflow_from_config` | Return types |
| `comfy_workflow.rs` | `ComfyWorkflow` struct | Field changes |
| `comfy_client.rs` | `ComfyUIClient::test_connection` | Method signature |

## Features

- **PNG Import**: Extract workflow from ComfyUI-generated images
- **JSON Import**: Load workflow JSON directly
- **Node Discovery**: Auto-detect controllable inputs
- **Agent Access**: Toggle which inputs agent can modify
- **Preview Image**: Show workflow's sample output
- **Connection Test**: Verify ComfyUI is reachable
- **Persistence**: Save to config as JSON string

## Notes
- Workflow name derived from import filename
- Controllable inputs show type and current value
- Checkbox toggles agent_modifiable flag
- Test uses blocking Tokio runtime for UI thread
- Import errors displayed in red, test status in green/red
