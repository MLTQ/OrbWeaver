use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct LlmClient {
    api_url: String,
    api_key: String,
    model: String,
    client: reqwest::Client,
}

#[derive(Debug, Deserialize)]
pub struct DecisionResponse {
    pub should_respond: bool,
    pub reasoning: String,
}

#[derive(Debug, Serialize)]
struct ChatCompletionRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Deserialize)]
struct ChatCompletionResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}

impl LlmClient {
    pub fn new(api_url: String, api_key: String, model: String) -> Self {
        Self {
            api_url,
            api_key,
            model,
            client: reqwest::Client::new(),
        }
    }

    /// Generate a completion using the OpenAI API format
    pub async fn generate(&self, messages: Vec<Message>) -> Result<String> {
        self.generate_with_model(messages, &self.model).await
    }

    /// Generate a completion with a specific model
    pub async fn generate_with_model(&self, messages: Vec<Message>, model: &str) -> Result<String> {
        let url = format!("{}/chat/completions", self.api_url);

        let request = ChatCompletionRequest {
            model: model.to_string(),
            messages,
            temperature: Some(0.7),
            max_tokens: Some(2000),
        };

        let mut req = self.client.post(&url).json(&request);

        // Add API key header if provided (not needed for local models)
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = req
            .send()
            .await
            .context("Failed to send LLM request")?;

        // Check for HTTP errors and include response body for debugging
        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
            anyhow::bail!("LLM API returned error {}: {}", status, body);
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .context("Failed to parse LLM response")?;

        let content = completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No response from LLM"))?;

        Ok(content)
    }

    /// Ask the LLM to decide whether to respond to a post
    pub async fn decide_to_respond(
        &self,
        messages: Vec<Message>,
        decision_model: Option<&str>,
    ) -> Result<DecisionResponse> {
        let model = decision_model.unwrap_or(&self.model);
        self.generate_json(messages, Some(model)).await
    }

    /// Generate a JSON response using the LLM
    pub async fn generate_json<T>(&self, messages: Vec<Message>, model: Option<&str>) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        let model = model.unwrap_or(&self.model);
        let response = self.generate_with_model(messages, model).await?;

        // Try to parse as JSON
        match serde_json::from_str::<T>(&response) {
            Ok(parsed) => Ok(parsed),
            Err(_) => {
                // If JSON parsing fails, try to extract from markdown code block
                let json_content = if let Some(start) = response.find("```json") {
                    let after_start = &response[start + 7..];
                    if let Some(end) = after_start.find("```") {
                        after_start[..end].trim()
                    } else {
                        &response
                    }
                } else if let Some(start) = response.find('{') {
                    if let Some(end) = response.rfind('}') {
                        &response[start..=end]
                    } else {
                        &response
                    }
                } else {
                    &response
                };

                serde_json::from_str::<T>(json_content)
                    .context(format!("Failed to parse JSON response. Raw response: {}", response))
            }
        }
    }
}
