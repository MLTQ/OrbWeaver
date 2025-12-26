use anyhow::{Context, Result};
use base64::Engine;
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

    /// Evaluate an image with vision model
    pub async fn evaluate_image(
        &self,
        image_bytes: &[u8],
        prompt: &str,
        context: &str,
    ) -> Result<ImageEvaluation> {
        // Encode image to base64
        let image_base64 = base64::engine::general_purpose::STANDARD.encode(image_bytes);

        let messages = vec![
            Message {
                role: "system".to_string(),
                content: "You are evaluating AI-generated images for an imageboard posting agent. \
                         Determine if the image matches the intended prompt and context, and suggest \
                         improvements if needed.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: format!(
                    "Context: {}\n\n\
                     Image Prompt: {}\n\n\
                     Evaluate this generated image. Does it match the prompt and fit the context? \
                     Should we post it, or generate a better one?\n\n\
                     Respond with JSON:\n\
                     {{\n  \
                       \"satisfactory\": true/false,\n  \
                       \"reasoning\": \"explanation\",\n  \
                       \"suggested_prompt_refinement\": \"improved prompt if not satisfactory, null otherwise\"\n\
                     }}",
                    context,
                    prompt
                ),
            },
        ];

        // For vision models, we need to send image data
        // This depends on the API format - adjust for your vision model
        let response = self.generate_vision_with_image(messages, &image_base64).await?;

        self.parse_json::<ImageEvaluation>(&response)
    }

    async fn generate_vision_with_image(
        &self,
        messages: Vec<Message>,
        image_base64: &str,
    ) -> Result<String> {
        // NOTE: This is a placeholder implementation for vision models
        // Most OpenAI-compatible APIs don't support vision in the standard format
        //
        // For actual vision support, you'll need to adapt this based on your provider:
        //
        // 1. OpenAI GPT-4 Vision:
        //    - Use "content" as array with text and image_url objects
        //    - image_url.url should be "data:image/jpeg;base64,{base64}"
        //
        // 2. Anthropic Claude with Vision:
        //    - Use "content" as array with text and image objects
        //    - image.source.data should be base64 string
        //
        // 3. Ollama llava/llama3.2-vision:
        //    - Add "images": [base64_string] to the request body
        //    - Keep messages.content as regular string
        //
        // For now, this sends base64 in the message text (likely to fail)
        let url = format!("{}/chat/completions", self.api_url);

        // Modify the last message to include image (placeholder approach)
        let mut messages = messages;
        if let Some(last_msg) = messages.last_mut() {
            last_msg.content = format!(
                "[IMAGE_BASE64: {}]\n\n{}",
                image_base64,
                last_msg.content
            );
        }

        let request = ChatCompletionRequest {
            model: self.model.clone(),
            messages,
            temperature: Some(0.7),
            max_tokens: Some(1000),
        };

        let mut req = self.client.post(&url).json(&request);

        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = req.send().await.context("Failed to send vision request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_else(|_| "Unable to read body".to_string());
            anyhow::bail!("Vision API returned error {}: {}", status, body);
        }

        let completion: ChatCompletionResponse = response
            .json()
            .await
            .context("Failed to parse vision response")?;

        let content = completion
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No response from vision model"))?;

        Ok(content)
    }

    fn parse_json<T>(&self, response: &str) -> Result<T>
    where
        T: for<'de> Deserialize<'de>,
    {
        // First try parsing the whole response
        match serde_json::from_str::<T>(response) {
            Ok(parsed) => return Ok(parsed),
            Err(_) => {}
        }

        // Strip <think> tags if present (some models like GLM output reasoning in <think> blocks)
        let cleaned = if let Some(think_end) = response.rfind("</think>") {
            &response[think_end + 8..]
        } else {
            response
        };

        // Try parsing cleaned content
        match serde_json::from_str::<T>(cleaned.trim()) {
            Ok(parsed) => return Ok(parsed),
            Err(_) => {}
        }

        // Extract JSON from code blocks or find first { to last }
        let json_content = if let Some(start) = cleaned.find("```json") {
            let after_start = &cleaned[start + 7..];
            if let Some(end) = after_start.find("```") {
                after_start[..end].trim()
            } else {
                cleaned
            }
        } else if let Some(start) = cleaned.find('{') {
            if let Some(end) = cleaned.rfind('}') {
                &cleaned[start..=end]
            } else {
                cleaned
            }
        } else {
            cleaned
        };

        serde_json::from_str::<T>(json_content.trim())
            .context(format!(
                "Failed to parse JSON. Extracted: {} | Original: {}",
                json_content,
                response.chars().take(500).collect::<String>()
            ))
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct ImageEvaluation {
    pub satisfactory: bool,
    pub reasoning: String,
    pub suggested_prompt_refinement: Option<String>,
}
