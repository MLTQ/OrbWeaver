use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

#[derive(Debug, Clone, Serialize)]
pub struct QueuePromptRequest {
    pub prompt: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct QueuePromptResponse {
    pub prompt_id: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct HistoryEntry {
    pub outputs: HashMap<String, OutputNode>,
    pub status: Option<StatusInfo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct OutputNode {
    pub images: Option<Vec<ImageInfo>>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ImageInfo {
    pub filename: String,
    pub subfolder: String,
    #[serde(rename = "type")]
    pub image_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StatusInfo {
    pub status_str: Option<String>,
    pub completed: Option<bool>,
}

/// Client for ComfyUI API
pub struct ComfyUIClient {
    api_url: String,
    client: Client,
}

impl ComfyUIClient {
    pub fn new(api_url: String) -> Self {
        Self {
            api_url,
            client: Client::new(),
        }
    }

    /// Queue a workflow for execution
    pub async fn queue_prompt(&self, workflow: serde_json::Value) -> Result<String> {
        let url = format!("{}/prompt", self.api_url);

        let request = QueuePromptRequest {
            prompt: workflow,
            client_id: None,
        };

        let response = self.client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send prompt to ComfyUI")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("ComfyUI API error {}: {}", status, body);
        }

        let result: QueuePromptResponse = response
            .json()
            .await
            .context("Failed to parse ComfyUI response")?;

        Ok(result.prompt_id)
    }

    /// Check the status of a queued prompt
    pub async fn get_history(&self, prompt_id: &str) -> Result<Option<HistoryEntry>> {
        let url = format!("{}/history/{}", self.api_url, prompt_id);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to get history from ComfyUI")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to get history: {}", response.status());
        }

        let history: HashMap<String, HistoryEntry> = response
            .json()
            .await
            .context("Failed to parse history response")?;

        Ok(history.get(prompt_id).cloned())
    }

    /// Wait for a prompt to complete execution
    pub async fn wait_for_completion(&self, prompt_id: &str, timeout_secs: u64) -> Result<ImageInfo> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(timeout_secs);

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Timeout waiting for ComfyUI to complete");
            }

            if let Some(history) = self.get_history(prompt_id).await? {
                // Check if completed
                if let Some(status) = &history.status {
                    if status.completed == Some(true) {
                        // Find output image
                        for (node_id, output) in &history.outputs {
                            if let Some(images) = &output.images {
                                if let Some(image) = images.first() {
                                    tracing::info!("Image generated: {} (node {})", image.filename, node_id);
                                    return Ok(image.clone()); // Clone to return owned value
                                }
                            }
                        }
                        anyhow::bail!("Workflow completed but no image found");
                    }
                }
            }

            // Poll every second
            sleep(Duration::from_secs(1)).await;
        }
    }

    /// Download generated image
    pub async fn download_image(&self, image_info: &ImageInfo) -> Result<PathBuf> {
        let url = format!(
            "{}/view?filename={}&subfolder={}&type={}",
            self.api_url,
            image_info.filename,
            image_info.subfolder,
            image_info.image_type
        );

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to download image from ComfyUI")?;

        if !response.status().is_success() {
            anyhow::bail!("Failed to download image: {}", response.status());
        }

        let bytes = response.bytes().await.context("Failed to read image bytes")?;

        // Save to temp file
        let output_path = PathBuf::from(format!("generated_{}", image_info.filename));
        std::fs::write(&output_path, bytes)
            .with_context(|| format!("Failed to write image to {:?}", output_path))?;

        Ok(output_path)
    }

    /// Test connection to ComfyUI
    pub async fn test_connection(&self) -> Result<()> {
        let url = format!("{}/history", self.api_url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to connect to ComfyUI")?;

        if !response.status().is_success() {
            anyhow::bail!("ComfyUI returned error: {}", response.status());
        }

        Ok(())
    }
}
