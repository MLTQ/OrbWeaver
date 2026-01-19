use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Types of inputs that can be controlled
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum InputType {
    Text,
    Int,
    Float,
    Bool,
    Seed,
}

/// A controllable input parameter in a workflow node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllableInput {
    pub name: String,
    pub input_type: InputType,
    pub default_value: serde_json::Value,
    pub agent_modifiable: bool,
    pub description: String,
}

/// A node in the workflow that has controllable inputs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ControllableNode {
    pub node_id: String,
    pub class_type: String,
    pub inputs: Vec<ControllableInput>,
}

/// Complete ComfyUI workflow with metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyWorkflow {
    /// Display name for this workflow
    pub name: String,

    /// Raw workflow JSON (the actual ComfyUI graph)
    pub workflow_json: serde_json::Value,

    /// Nodes that the agent or user can control
    pub controllable_nodes: HashMap<String, ControllableNode>,

    /// ID of the node that outputs the final image
    pub output_node_id: String,

    /// Optional preview/example image path
    pub preview_image_path: Option<String>,
}

impl ComfyWorkflow {
    /// Import workflow from ComfyUI PNG with embedded metadata
    pub fn from_png<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();
        let bytes = fs::read(path_ref)
            .with_context(|| format!("Failed to read PNG from {:?}", path_ref))?;

        // Extract workflow from PNG tEXt chunks
        let workflow_json = extract_comfy_workflow_from_png(&bytes)?;

        // Parse the workflow
        let mut workflow = Self::from_workflow_json(workflow_json)?;

        // Store preview image path
        workflow.preview_image_path = Some(path_ref.to_string_lossy().to_string());

        Ok(workflow)
    }

    /// Import workflow from JSON file
    pub fn from_json_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read workflow JSON from {:?}", path.as_ref()))?;

        let workflow_json: serde_json::Value = serde_json::from_str(&contents)
            .context("Failed to parse workflow JSON")?;

        Self::from_workflow_json(workflow_json)
    }

    /// Create workflow from parsed JSON
    fn from_workflow_json(workflow_json: serde_json::Value) -> Result<Self> {
        // Detect controllable nodes
        let controllable_nodes = detect_controllable_nodes(&workflow_json)?;

        // Find output node (SaveImage or PreviewImage)
        let output_node_id = find_output_node(&workflow_json)?;

        Ok(Self {
            name: "Imported Workflow".to_string(),
            workflow_json,
            controllable_nodes,
            output_node_id,
            preview_image_path: None,
        })
    }

    /// Prepare workflow for execution with agent-provided inputs
    pub fn prepare_for_execution(&self, inputs: &HashMap<String, serde_json::Value>) -> Result<serde_json::Value> {
        let mut workflow = self.workflow_json.clone();

        // Apply user/agent inputs to controllable nodes
        for (node_id, node_info) in &self.controllable_nodes {
            if let Some(workflow_node) = workflow.get_mut(node_id) {
                for input in &node_info.inputs {
                    if input.agent_modifiable {
                        // Check if agent provided a value
                        if let Some(value) = inputs.get(&input.name) {
                            // Update the workflow node's input
                            if let Some(inputs_obj) = workflow_node.get_mut("inputs") {
                                inputs_obj[&input.name] = value.clone();
                            }
                        }
                    }
                }
            }
        }

        Ok(workflow)
    }
}

/// Extract ComfyUI workflow from PNG tEXt chunk
fn extract_comfy_workflow_from_png(png_bytes: &[u8]) -> Result<serde_json::Value> {
    // PNG signature check
    if png_bytes.len() < 8 || &png_bytes[0..8] != b"\x89PNG\r\n\x1a\n" {
        anyhow::bail!("Not a valid PNG file");
    }

    let mut pos = 8;

    // Look for "workflow" or "prompt" tEXt chunks
    let mut workflow_json: Option<String> = None;

    while pos + 12 <= png_bytes.len() {
        let length = u32::from_be_bytes([
            png_bytes[pos],
            png_bytes[pos + 1],
            png_bytes[pos + 2],
            png_bytes[pos + 3],
        ]) as usize;

        let chunk_type = &png_bytes[pos + 4..pos + 8];

        if chunk_type == b"tEXt" {
            let chunk_data = &png_bytes[pos + 8..pos + 8 + length];

            if let Some(null_pos) = chunk_data.iter().position(|&b| b == 0) {
                let keyword = std::str::from_utf8(&chunk_data[0..null_pos]).unwrap_or("");

                // ComfyUI stores workflow in "workflow" or "prompt" chunks
                if keyword == "workflow" || keyword == "prompt" {
                    let text_data = &chunk_data[null_pos + 1..];
                    workflow_json = Some(String::from_utf8(text_data.to_vec())
                        .context("Workflow data is not valid UTF-8")?);
                    break;
                }
            }
        }

        pos += 12 + length;
    }

    let json_str = workflow_json
        .ok_or_else(|| anyhow::anyhow!("No ComfyUI workflow found in PNG metadata"))?;

    serde_json::from_str(&json_str)
        .context("Failed to parse workflow JSON from PNG")
}

/// Auto-detect controllable nodes in workflow
fn detect_controllable_nodes(workflow: &serde_json::Value) -> Result<HashMap<String, ControllableNode>> {
    let mut controllable = HashMap::new();

    if let Some(nodes) = workflow.as_object() {
        for (node_id, node) in nodes {
            if let Some(class_type) = node.get("class_type").and_then(|v| v.as_str()) {
                let mut inputs = Vec::new();

                match class_type {
                    "CLIPTextEncode" => {
                        // This is a prompt node
                        if let Some(node_inputs) = node.get("inputs") {
                            if let Some(text) = node_inputs.get("text") {
                                inputs.push(ControllableInput {
                                    name: "text".to_string(),
                                    input_type: InputType::Text,
                                    default_value: text.clone(),
                                    agent_modifiable: true,
                                    description: "Prompt text".to_string(),
                                });
                            }
                        }
                    }
                    "KSampler" | "KSamplerAdvanced" => {
                        // Sampler node - seed, steps, cfg, denoise
                        if let Some(node_inputs) = node.get("inputs") {
                            if let Some(seed) = node_inputs.get("seed") {
                                inputs.push(ControllableInput {
                                    name: "seed".to_string(),
                                    input_type: InputType::Seed,
                                    default_value: seed.clone(),
                                    agent_modifiable: true,
                                    description: "Random seed (-1 for random)".to_string(),
                                });
                            }
                            if let Some(steps) = node_inputs.get("steps") {
                                inputs.push(ControllableInput {
                                    name: "steps".to_string(),
                                    input_type: InputType::Int,
                                    default_value: steps.clone(),
                                    agent_modifiable: false,
                                    description: "Sampling steps".to_string(),
                                });
                            }
                            if let Some(cfg) = node_inputs.get("cfg") {
                                inputs.push(ControllableInput {
                                    name: "cfg".to_string(),
                                    input_type: InputType::Float,
                                    default_value: cfg.clone(),
                                    agent_modifiable: false,
                                    description: "CFG scale".to_string(),
                                });
                            }
                            if let Some(denoise) = node_inputs.get("denoise") {
                                inputs.push(ControllableInput {
                                    name: "denoise".to_string(),
                                    input_type: InputType::Float,
                                    default_value: denoise.clone(),
                                    agent_modifiable: false,
                                    description: "Denoise strength".to_string(),
                                });
                            }
                        }
                    }
                    "EmptyLatentImage" => {
                        // Latent size
                        if let Some(node_inputs) = node.get("inputs") {
                            if let Some(width) = node_inputs.get("width") {
                                inputs.push(ControllableInput {
                                    name: "width".to_string(),
                                    input_type: InputType::Int,
                                    default_value: width.clone(),
                                    agent_modifiable: false,
                                    description: "Image width".to_string(),
                                });
                            }
                            if let Some(height) = node_inputs.get("height") {
                                inputs.push(ControllableInput {
                                    name: "height".to_string(),
                                    input_type: InputType::Int,
                                    default_value: height.clone(),
                                    agent_modifiable: false,
                                    description: "Image height".to_string(),
                                });
                            }
                        }
                    }
                    _ => {}
                }

                if !inputs.is_empty() {
                    controllable.insert(
                        node_id.clone(),
                        ControllableNode {
                            node_id: node_id.clone(),
                            class_type: class_type.to_string(),
                            inputs,
                        },
                    );
                }
            }
        }
    }

    Ok(controllable)
}

/// Find the output node in the workflow
fn find_output_node(workflow: &serde_json::Value) -> Result<String> {
    if let Some(nodes) = workflow.as_object() {
        for (node_id, node) in nodes {
            if let Some(class_type) = node.get("class_type").and_then(|v| v.as_str()) {
                if class_type == "SaveImage" || class_type == "PreviewImage" {
                    return Ok(node_id.clone());
                }
            }
        }
    }

    anyhow::bail!("No output node (SaveImage/PreviewImage) found in workflow")
}
