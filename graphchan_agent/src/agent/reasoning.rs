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
        // Extract and clean JSON from LLM response
        let cleaned_json = extract_json(llm_response)?;

        // Try to parse as JSON
        let json_response: serde_json::Value = serde_json::from_str(&cleaned_json)
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

/// Extract JSON from LLM response, handling common formatting issues
fn extract_json(response: &str) -> Result<String> {
    let trimmed = response.trim();

    // Case 1: Check for markdown code blocks (```json ... ``` or ``` ... ```)
    if let Some(json) = extract_from_markdown_code_block(trimmed) {
        return Ok(json);
    }

    // Case 2: Try to find JSON object/array by braces/brackets
    if let Some(json) = extract_by_delimiters(trimmed) {
        return Ok(json);
    }

    // Case 3: Try parsing as-is (maybe it's already clean JSON)
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Ok(trimmed.to_string());
    }

    // Case 4: Last resort - try to clean common issues
    let cleaned = clean_json_string(trimmed);
    if serde_json::from_str::<serde_json::Value>(&cleaned).is_ok() {
        return Ok(cleaned);
    }

    anyhow::bail!("Could not extract valid JSON from LLM response: {}", response)
}

/// Extract JSON from markdown code blocks like ```json ... ``` or ``` ... ```
fn extract_from_markdown_code_block(text: &str) -> Option<String> {
    // Try ```json ... ```
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            let json = text[start + 7..start + 7 + end].trim();
            return Some(json.to_string());
        }
    }

    // Try ``` ... ``` (generic code block)
    if let Some(start) = text.find("```") {
        if let Some(end) = text[start + 3..].find("```") {
            let json = text[start + 3..start + 3 + end].trim();
            // Verify it looks like JSON
            if json.starts_with('{') || json.starts_with('[') {
                return Some(json.to_string());
            }
        }
    }

    None
}

/// Extract JSON by finding matching braces/brackets
fn extract_by_delimiters(text: &str) -> Option<String> {
    // Try to find JSON object {...}
    if let Some(start) = text.find('{') {
        if let Some(json) = extract_balanced_braces(&text[start..], '{', '}') {
            return Some(json);
        }
    }

    // Try to find JSON array [...]
    if let Some(start) = text.find('[') {
        if let Some(json) = extract_balanced_braces(&text[start..], '[', ']') {
            return Some(json);
        }
    }

    None
}

/// Extract text between balanced delimiters
fn extract_balanced_braces(text: &str, open: char, close: char) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut depth = 0;
    let mut start = None;
    let mut end = None;

    for (i, &ch) in chars.iter().enumerate() {
        if ch == open {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if ch == close {
            depth -= 1;
            if depth == 0 && start.is_some() {
                end = Some(i);
                break;
            }
        }
    }

    if let (Some(s), Some(e)) = (start, end) {
        let result: String = chars[s..=e].iter().collect();
        // Verify it's valid JSON before returning
        if serde_json::from_str::<serde_json::Value>(&result).is_ok() {
            return Some(result);
        }
    }

    None
}

/// Clean common JSON formatting issues
fn clean_json_string(text: &str) -> String {
    // Remove common markdown/formatting around JSON
    let mut cleaned = text
        .trim_start_matches("json")
        .trim_start_matches("JSON")
        .trim()
        .to_string();

    // Remove trailing commas before closing braces/brackets (common LLM mistake)
    cleaned = cleaned.replace(",}", "}");
    cleaned = cleaned.replace(",]", "]");

    // Remove comments (// and /* */ style - not valid in JSON but LLMs sometimes add them)
    cleaned = remove_comments(&cleaned);

    // Fix common quote issues - replace smart quotes with regular quotes
    cleaned = cleaned.replace('\u{201C}', "\"").replace('\u{201D}', "\""); // " and "
    cleaned = cleaned.replace('\u{2018}', "'").replace('\u{2019}', "'"); // ' and '

    cleaned
}

/// Remove C-style comments from JSON string
fn remove_comments(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    let mut in_string = false;
    let mut escape_next = false;

    while let Some(ch) = chars.next() {
        if escape_next {
            result.push(ch);
            escape_next = false;
            continue;
        }

        if ch == '\\' && in_string {
            result.push(ch);
            escape_next = true;
            continue;
        }

        if ch == '"' {
            in_string = !in_string;
            result.push(ch);
            continue;
        }

        if !in_string && ch == '/' {
            if let Some(&next_ch) = chars.peek() {
                if next_ch == '/' {
                    // Single-line comment - skip until newline
                    chars.next(); // consume second /
                    while let Some(c) = chars.next() {
                        if c == '\n' {
                            result.push(c);
                            break;
                        }
                    }
                    continue;
                } else if next_ch == '*' {
                    // Multi-line comment - skip until */
                    chars.next(); // consume *
                    let mut prev = ' ';
                    while let Some(c) = chars.next() {
                        if prev == '*' && c == '/' {
                            break;
                        }
                        prev = c;
                    }
                    continue;
                }
            }
        }

        result.push(ch);
    }

    result
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_json_from_markdown() {
        let input = r#"Sure! Here's the JSON:
```json
{"action": "reply", "post_id": "123", "content": "test", "reasoning": ["step1"]}
```
That should work!"#;

        let result = extract_json(input).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn test_extract_json_from_generic_code_block() {
        let input = r#"Here you go:
```
{"action": "none", "reasoning": ["no interesting posts"]}
```"#;

        let result = extract_json(input).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn test_extract_json_with_text_before_and_after() {
        let input = r#"I think the best response is: {"action": "reply", "post_id": "abc", "content": "Great point!", "reasoning": ["relevant"]} and that's my decision."#;

        let result = extract_json(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["action"], "reply");
    }

    #[test]
    fn test_extract_json_with_trailing_commas() {
        let input = r#"{"action": "none", "reasoning": ["step1", "step2",],}"#;

        let result = extract_json(input).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn test_extract_json_with_comments() {
        let input = r#"{
            "action": "reply", // This is the action
            "post_id": "123",
            /* This is the content */
            "content": "test",
            "reasoning": ["step1"]
        }"#;

        let result = extract_json(input).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn test_extract_json_with_smart_quotes() {
        let input = r#"{"action": "reply", "content": "Here's a test", "post_id": "123", "reasoning": []}"#;

        let result = extract_json(input).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn test_clean_json() {
        let input = "Here is my response: {\"action\": \"none\", \"reasoning\": []}";
        let result = extract_json(input).unwrap();
        assert!(serde_json::from_str::<serde_json::Value>(&result).is_ok());
    }

    #[test]
    fn test_nested_braces() {
        let input = r#"{"action": "reply", "metadata": {"nested": "value"}, "reasoning": ["test"]}"#;

        let result = extract_json(input).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["metadata"]["nested"], "value");
    }
}
