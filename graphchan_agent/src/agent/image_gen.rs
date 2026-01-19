use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::comfy_client::ComfyUIClient;
use crate::comfy_workflow::ComfyWorkflow;

/// Image generation handler for the agent
pub struct ImageGenerator {
    client: ComfyUIClient,
    workflow: Option<ComfyWorkflow>,
}

impl ImageGenerator {
    pub fn new(comfy_api_url: String, workflow: Option<ComfyWorkflow>) -> Self {
        Self {
            client: ComfyUIClient::new(comfy_api_url),
            workflow,
        }
    }

    /// Check if image generation is available
    pub fn is_available(&self) -> bool {
        self.workflow.is_some()
    }

    /// Generate an image based on a prompt and context
    pub async fn generate_image(&self, prompt: &str) -> Result<PathBuf> {
        let workflow = self.workflow.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No workflow configured"))?;

        // Prepare inputs for the workflow
        let mut inputs = HashMap::new();

        // Find text inputs and populate with prompt
        for (node_id, node) in &workflow.controllable_nodes {
            for input in &node.inputs {
                if input.agent_modifiable {
                    match input.name.as_str() {
                        "text" => {
                            // This is likely the positive prompt
                            inputs.insert(format!("{}_text", node_id), serde_json::json!(prompt));
                        }
                        "seed" => {
                            // Use random seed
                            inputs.insert(format!("{}_seed", node_id), serde_json::json!(-1));
                        }
                        _ => {}
                    }
                }
            }
        }

        // Prepare workflow for execution
        let executable_workflow = workflow.prepare_for_execution(&inputs)?;

        // Queue prompt
        let prompt_id = self.client.queue_prompt(executable_workflow).await
            .context("Failed to queue prompt")?;

        tracing::info!("Queued image generation: prompt_id={}", prompt_id);

        // Wait for completion (timeout: 5 minutes)
        let image_info = self.client.wait_for_completion(&prompt_id, 300).await
            .context("Failed to wait for completion")?;

        // Download image
        let image_path = self.client.download_image(&image_info).await
            .context("Failed to download generated image")?;

        tracing::info!("Generated image saved to: {:?}", image_path);

        Ok(image_path)
    }

    /// Generate a prompt from thread context using LLM
    pub async fn generate_prompt_from_context(&self, context: &str, llm_client: &crate::agent::reasoning::ReasoningEngine) -> Result<String> {
        // For now, use a simple heuristic
        // TODO: Use LLM to craft better prompts from context

        let prompt = if context.len() < 100 {
            format!("A visual representation of: {}", context)
        } else {
            format!("A visual representation of the following discussion: {}...", &context[..100])
        };

        Ok(prompt)
    }
}
