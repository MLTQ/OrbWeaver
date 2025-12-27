use anyhow::{Context, Result};
use log::info;
use serde::Deserialize;
use serde_json::json;
use std::time::Duration;
use crate::config::ComfyUIConfig;

#[derive(Clone)]
pub struct ComfyUIClient {
    api_url: String,
    config: ComfyUIConfig,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
struct PromptResponse {
    prompt_id: String,
}

#[derive(Debug, Deserialize)]
struct HistoryResponse {
    #[serde(flatten)]
    prompts: std::collections::HashMap<String, PromptHistory>,
}

#[derive(Debug, Deserialize)]
struct PromptHistory {
    outputs: std::collections::HashMap<String, NodeOutput>,
}

#[derive(Debug, Deserialize)]
struct NodeOutput {
    images: Option<Vec<ImageOutput>>,
}

#[derive(Debug, Clone, Deserialize)]
struct ImageOutput {
    filename: String,
    subfolder: String,
    #[serde(rename = "type")]
    output_type: String,
}

impl ComfyUIClient {
    pub fn new(config: ComfyUIConfig) -> Self {
        let api_url = config.api_url.clone();
        Self {
            api_url,
            config,
            client: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .expect("Failed to create HTTP client"),
        }
    }

    /// Generate an image from a prompt (MINIMAL VERSION)
    pub async fn generate_image(
        &self,
        prompt: &str,
        negative_prompt: Option<&str>,
    ) -> Result<Vec<u8>> {
        // Build minimal workflow
        let workflow = self.build_minimal_workflow(prompt, negative_prompt)?;

        // Submit prompt
        let url = format!("{}/prompt", self.api_url);
        let client_id = uuid::Uuid::new_v4().to_string();

        let request_body = json!({
            "prompt": workflow,
            "client_id": client_id
        });

        let response = self.client
            .post(&url)
            .json(&request_body)
            .send()
            .await
            .context("Failed to submit prompt to ComfyUI")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
            anyhow::bail!("ComfyUI returned error {}: {}", status, body);
        }

        let response_data: PromptResponse = response
            .json()
            .await
            .context("Failed to parse ComfyUI response")?;

        let prompt_id = response_data.prompt_id;
        info!("Submitted prompt to ComfyUI: {}", prompt_id);

        // Poll until complete
        let image_info = self.poll_until_complete(&prompt_id).await?;

        // Download image
        let image_bytes = self.download_image(&image_info).await?;

        Ok(image_bytes)
    }

    fn build_minimal_workflow(
        &self,
        prompt: &str,
        negative_prompt: Option<&str>,
    ) -> Result<serde_json::Value> {
        // Minimal SD/SDXL workflow (works for both)
        let seed = rand::random::<u32>() as i64;

        // Determine which VAE to use: dedicated loader or checkpoint's VAE
        let vae_input = if self.config.vae_name.is_some() {
            // Use dedicated VAE loader (node 10)
            json!(["10", 0])
        } else {
            // Use VAE from checkpoint
            json!(["4", 2])
        };

        let mut workflow = json!({
            "3": {
                "inputs": {
                    "seed": seed,
                    "steps": self.config.steps,
                    "cfg": self.config.cfg_scale,
                    "sampler_name": self.config.sampler,
                    "scheduler": self.config.scheduler,
                    "denoise": 1.0,
                    "model": ["4", 0],
                    "positive": ["6", 0],
                    "negative": ["7", 0],
                    "latent_image": ["5", 0]
                },
                "class_type": "KSampler"
            },
            "4": {
                "inputs": {
                    "ckpt_name": self.config.model_name
                },
                "class_type": "CheckpointLoaderSimple"
            },
            "5": {
                "inputs": {
                    "width": self.config.width,
                    "height": self.config.height,
                    "batch_size": 1
                },
                "class_type": "EmptyLatentImage"
            },
            "6": {
                "inputs": {
                    "text": prompt,
                    "clip": ["4", 1]
                },
                "class_type": "CLIPTextEncode"
            },
            "7": {
                "inputs": {
                    "text": negative_prompt.unwrap_or(""),
                    "clip": ["4", 1]
                },
                "class_type": "CLIPTextEncode"
            },
            "8": {
                "inputs": {
                    "samples": ["3", 0],
                    "vae": vae_input
                },
                "class_type": "VAEDecode"
            },
            "9": {
                "inputs": {
                    "filename_prefix": "graphchan_agent",
                    "images": ["8", 0]
                },
                "class_type": "SaveImage"
            }
        });

        // Add VAELoader node if a specific VAE is configured
        if let Some(vae_name) = &self.config.vae_name {
            workflow["10"] = json!({
                "inputs": {
                    "vae_name": vae_name
                },
                "class_type": "VAELoader"
            });
        }

        Ok(workflow)
    }

    async fn poll_until_complete(&self, prompt_id: &str) -> Result<ImageOutput> {
        let url = format!("{}/history/{}", self.api_url, prompt_id);

        for attempt in 1..=60 {
            tokio::time::sleep(Duration::from_secs(2)).await;

            let response = self.client
                .get(&url)
                .send()
                .await
                .context("Failed to poll ComfyUI history")?;

            if !response.status().is_success() {
                continue;
            }

            let history: HistoryResponse = response
                .json()
                .await
                .context("Failed to parse history response")?;

            if let Some(prompt_history) = history.prompts.get(prompt_id) {
                // Look for SaveImage node output (node "9")
                if let Some(node_output) = prompt_history.outputs.get("9") {
                    if let Some(images) = &node_output.images {
                        if let Some(image) = images.first() {
                            info!("Image generation complete: {}", image.filename);
                            return Ok(image.clone());
                        }
                    }
                }
            }

            if attempt % 10 == 0 {
                info!("Still waiting for image generation... ({}/60)", attempt);
            }
        }

        anyhow::bail!("Timeout waiting for image generation")
    }

    async fn download_image(&self, image_info: &ImageOutput) -> Result<Vec<u8>> {
        let url = format!(
            "{}/view?filename={}&subfolder={}&type={}",
            self.api_url,
            image_info.filename,
            image_info.subfolder,
            image_info.output_type
        );

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to download image from ComfyUI")?;

        if !response.status().is_success() {
            let status = response.status();
            anyhow::bail!("Failed to download image: {}", status);
        }

        let bytes = response.bytes().await.context("Failed to read image bytes")?;
        Ok(bytes.to_vec())
    }

    /// Get prompt style based on workflow type
    pub fn prompt_style(&self) -> &'static str {
        match self.config.workflow_type {
            crate::config::WorkflowType::SD | crate::config::WorkflowType::SDXL => "tags",
            crate::config::WorkflowType::Flux => "natural",
        }
    }

    pub fn workflow_type(&self) -> &crate::config::WorkflowType {
        &self.config.workflow_type
    }

    pub fn negative_prompt(&self) -> &str {
        &self.config.negative_prompt
    }
}
