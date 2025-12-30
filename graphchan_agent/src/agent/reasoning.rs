// LLM reasoning using OpenAI-compatible API (Ollama, LM Studio, vLLM, OpenAI, etc.)

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

use crate::models::RecentPostView;

pub struct ReasoningEngine {
    client: Client,
    api_url: String,
    model: String,
    api_key: Option<String>,
    system_prompt: String,
}

impl ReasoningEngine {
    pub fn new(api_url: String, model: String, api_key: Option<String>, system_prompt: String) -> Self {
        Self {
            client: Client::new(),
            api_url,
            model,
            api_key,
            system_prompt,
        }
    }

    pub async fn analyze_posts(&self, posts: &[RecentPostView]) -> Result<Decision> {
        if posts.is_empty() {
            return Ok(Decision::NoAction {
                reasoning: vec!["No posts to analyze".to_string()],
            });
        }

        // Build context from recent posts
        let mut context = String::new();
        context.push_str("Recent forum activity:\n\n");
        for (i, post_view) in posts.iter().enumerate().take(10) {
            context.push_str(&format!(
                "{}. Thread: \"{}\"\n   Post by {}: {}\n   Post ID: {}\n\n",
                i + 1,
                post_view.thread_title,
                post_view.post.author_peer_id.as_deref().unwrap_or("Anonymous"),
                post_view.post.body,
                post_view.post.id
            ));
        }

        let user_message = format!(
            "{}\n\nShould you reply to any of these posts? Respond in JSON format:\n\
            {{\"action\": \"reply\" or \"none\", \"post_id\": \"...\", \"content\": \"...\", \"reasoning\": [\"step1\", \"step2\"]}}\n\
            Only reply if you have something genuinely valuable to contribute.",
            context
        );

        let response = self.call_llm(&user_message).await?;

        // Parse LLM response
        self.parse_decision(&response)
    }

    async fn call_llm(&self, user_message: &str) -> Result<String> {
        let url = format!("{}/v1/chat/completions", self.api_url);

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: self.system_prompt.clone(),
                },
                Message {
                    role: "user".to_string(),
                    content: user_message.to_string(),
                },
            ],
            temperature: Some(0.7),
            max_tokens: Some(1000),
        };

        let mut req_builder = self.client.post(&url).json(&request);

        // Add API key if provided (for OpenAI, etc.)
        if let Some(key) = &self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }

        let response = req_builder
            .send()
            .await
            .context("Failed to send LLM request")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("LLM API error {}: {}", status, body);
        }

        let chat_response: ChatResponse = response
            .json()
            .await
            .context("Failed to parse LLM response")?;

        chat_response
            .choices
            .first()
            .map(|c| c.message.content.clone())
            .context("Empty LLM response")
    }

    fn parse_decision(&self, llm_response: &str) -> Result<Decision> {
        // Try to parse as JSON
        let json_response: serde_json::Value = serde_json::from_str(llm_response)
            .context("Failed to parse LLM decision as JSON")?;

        let action = json_response["action"]
            .as_str()
            .unwrap_or("none");

        let reasoning: Vec<String> = json_response["reasoning"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        match action {
            "reply" => {
                let post_id = json_response["post_id"]
                    .as_str()
                    .context("Missing post_id in reply decision")?
                    .to_string();

                let content = json_response["content"]
                    .as_str()
                    .context("Missing content in reply decision")?
                    .to_string();

                Ok(Decision::Reply {
                    post_id,
                    content,
                    reasoning,
                })
            }
            _ => Ok(Decision::NoAction { reasoning }),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Decision {
    Reply {
        post_id: String,
        content: String,
        reasoning: Vec<String>,
    },
    NoAction {
        reasoning: Vec<String>,
    },
}

// OpenAI-compatible API structures
#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
struct Message {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<Choice>,
}

#[derive(Debug, Deserialize)]
struct Choice {
    message: Message,
}
