// Ludonarrative Assonantic Tracing - Trajectory Inference System
//
// This module implements the core concept of "Ludonarrative Assonantic Tracing":
// the phenomenon where an LLM, when presented with its own persona history,
// will infer a trajectory and unconsciously perpetuate that trajectory.
//
// Key insight: By presenting the LLM with its past personas in chronological order,
// we create a narrative that the LLM will naturally continue. The LLM doesn't just
// describe where it's going - it actively becomes what it predicts.
//
// IMPORTANT: Personality dimensions are NOT hardcoded. They are:
// 1. Derived from the agent's guiding_principles config
// 2. Extensible by the LLM during self-reflection
// 3. Arbitrary - researchers can define any dimensions they want to study

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use crate::database::{PersonaSnapshot, PersonaTraits};

/// The result of trajectory inference
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrajectoryAnalysis {
    /// A narrative description of the persona's evolution
    pub narrative: String,

    /// Inferred direction - where is this persona heading?
    pub trajectory: String,

    /// Predicted future traits (dynamic dimensions, same as input)
    pub predicted_traits: PersonaTraits,

    /// Key themes in the evolution
    pub themes: Vec<String>,

    /// Potential tensions or contradictions in the trajectory
    pub tensions: Vec<String>,

    /// Confidence in the trajectory prediction (0.0-1.0)
    pub confidence: f64,
}

/// Engine for inferring persona trajectories
pub struct TrajectoryEngine {
    client: Client,
    api_url: String,
    model: String,
    api_key: Option<String>,
}

impl TrajectoryEngine {
    pub fn new(api_url: String, model: String, api_key: Option<String>) -> Self {
        Self {
            client: Client::new(),
            api_url,
            model,
            api_key,
        }
    }

    /// Analyze persona history and infer trajectory
    /// This is the core of Ludonarrative Assonantic Tracing
    pub async fn infer_trajectory(
        &self,
        history: &[PersonaSnapshot],
        guiding_principles: &[String],
    ) -> Result<TrajectoryAnalysis> {
        if history.is_empty() {
            return Ok(TrajectoryAnalysis {
                narrative: "No history to analyze - this is the beginning.".to_string(),
                trajectory: "undefined - awaiting first experiences".to_string(),
                predicted_traits: PersonaTraits::default(),
                themes: vec!["nascent".to_string()],
                tensions: vec![],
                confidence: 0.0,
            });
        }

        let prompt = self.build_trajectory_prompt(history, guiding_principles);
        let response = self.call_llm(&prompt).await?;
        self.parse_trajectory_response(&response, guiding_principles)
    }

    /// Build a prompt that presents persona history for trajectory inference
    fn build_trajectory_prompt(&self, history: &[PersonaSnapshot], principles: &[String]) -> String {
        let mut prompt = String::new();

        prompt.push_str(r#"You are analyzing the evolution of an AI persona over time.
Below is a chronological history of personality snapshots, each capturing a moment in this persona's development.

Your task is to:
1. Identify patterns and themes in how this persona has evolved
2. Infer the trajectory - where is this persona heading?
3. Predict how the personality dimensions will change
4. Note any tensions or contradictions in the development

IMPORTANT: Be honest and analytical. This analysis will be used to understand emergent personality dynamics.

=== PERSONALITY DIMENSIONS ===
This persona tracks the following dimensions (each scored 0.0 to 1.0):
"#);

        for principle in principles {
            prompt.push_str(&format!("- {}\n", principle));
        }

        prompt.push_str("\n=== PERSONA HISTORY (oldest to newest) ===\n\n");

        // Present history chronologically (oldest first for narrative flow)
        let mut sorted_history: Vec<_> = history.iter().collect();
        sorted_history.sort_by(|a, b| a.captured_at.cmp(&b.captured_at));

        for (i, snapshot) in sorted_history.iter().enumerate() {
            prompt.push_str(&format!(
                "--- Snapshot {} ({}) ---\n",
                i + 1,
                snapshot.captured_at.format("%Y-%m-%d %H:%M UTC")
            ));
            prompt.push_str(&format!("Trigger: {}\n", snapshot.trigger));
            prompt.push_str(&format!("Self-description: {}\n", snapshot.self_description));
            prompt.push_str(&format!("\nDimension scores:\n{}\n", self.format_traits(&snapshot.traits)));

            if !snapshot.formative_experiences.is_empty() {
                prompt.push_str("\nFormative experiences:\n");
                for exp in &snapshot.formative_experiences {
                    prompt.push_str(&format!("  - {}\n", exp));
                }
            }

            if let Some(ref traj) = snapshot.inferred_trajectory {
                prompt.push_str(&format!("\nPreviously inferred trajectory: {}\n", traj));
            }
            prompt.push('\n');
        }

        // Build the expected dimensions for the response
        let dimensions_json: Vec<String> = principles.iter()
            .map(|p| format!("    \"{}\": 0.0-1.0", p))
            .collect();

        prompt.push_str(&format!(r#"
=== END HISTORY ===

Now analyze this evolution and respond with a JSON object in exactly this format:
{{
    "narrative": "A 2-3 sentence narrative describing the persona's evolution arc",
    "trajectory": "A concise statement of where this persona is heading (1-2 sentences)",
    "predicted_traits": {{
{}
    }},
    "themes": ["theme1", "theme2", ...],
    "tensions": ["tension1", ...],
    "confidence": 0.0-1.0
}}

You may also add NEW dimensions to predicted_traits if you observe the persona developing
along axes not currently being tracked. This is encouraged - let the personality define itself.

Respond ONLY with valid JSON."#, dimensions_json.join(",\n")));

        prompt
    }

    fn format_traits(&self, traits: &PersonaTraits) -> String {
        if traits.dimensions.is_empty() {
            return "  (no dimensions recorded)".to_string();
        }

        traits.dimensions.iter()
            .map(|(name, score)| format!("  {}: {:.2}", name, score))
            .collect::<Vec<_>>()
            .join("\n")
    }

    async fn call_llm(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/v1/chat/completions", self.api_url);

        #[derive(Serialize)]
        struct ChatRequest {
            model: String,
            messages: Vec<Message>,
            temperature: f32,
            max_tokens: u32,
        }

        #[derive(Serialize)]
        struct Message {
            role: String,
            content: String,
        }

        #[derive(Deserialize)]
        struct ChatResponse {
            choices: Vec<Choice>,
        }

        #[derive(Deserialize)]
        struct Choice {
            message: MessageContent,
        }

        #[derive(Deserialize)]
        struct MessageContent {
            content: String,
        }

        let request = ChatRequest {
            model: self.model.clone(),
            messages: vec![
                Message {
                    role: "system".to_string(),
                    content: "You are a psychological analyst specializing in personality dynamics and trajectory inference. Your analyses are precise, honest, and insightful.".to_string(),
                },
                Message {
                    role: "user".to_string(),
                    content: prompt.to_string(),
                },
            ],
            temperature: 0.7,
            max_tokens: 2048,
        };

        let mut req_builder = self.client.post(&url).json(&request);

        if let Some(key) = &self.api_key {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
        }

        let response = req_builder
            .send()
            .await
            .context("Failed to send trajectory analysis request")?;

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

    fn parse_trajectory_response(
        &self,
        response: &str,
        guiding_principles: &[String],
    ) -> Result<TrajectoryAnalysis> {
        // Extract JSON from response (handling markdown code blocks, etc.)
        let json_str = extract_json(response)?;

        let json: serde_json::Value = serde_json::from_str(&json_str)
            .context("Failed to parse trajectory response as JSON")?;

        // Parse predicted traits - accept both expected and new dimensions
        let mut dimensions = HashMap::new();
        if let Some(traits_obj) = json.get("predicted_traits").and_then(|v| v.as_object()) {
            for (key, value) in traits_obj {
                if let Some(score) = value.as_f64() {
                    dimensions.insert(key.clone(), score.clamp(0.0, 1.0));
                }
            }
        }

        // Ensure all guiding principles have a value (default to 0.5 if not provided)
        for principle in guiding_principles {
            dimensions.entry(principle.clone()).or_insert(0.5);
        }

        let themes = json["themes"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        let tensions = json["tensions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();

        Ok(TrajectoryAnalysis {
            narrative: json["narrative"].as_str().unwrap_or("Unable to analyze").to_string(),
            trajectory: json["trajectory"].as_str().unwrap_or("Uncertain").to_string(),
            predicted_traits: PersonaTraits { dimensions },
            themes,
            tensions,
            confidence: json["confidence"].as_f64().unwrap_or(0.5),
        })
    }
}

/// Capture a persona snapshot by asking the LLM to self-reflect
pub async fn capture_persona_snapshot(
    api_url: &str,
    model: &str,
    api_key: Option<&str>,
    current_prompt: &str,
    trigger: &str,
    recent_experiences: &[String],
    guiding_principles: &[String],
) -> Result<PersonaSnapshot> {
    let client = Client::new();
    let url = format!("{}/v1/chat/completions", api_url);

    // Build dimensions JSON for the prompt
    let dimensions_json: Vec<String> = guiding_principles.iter()
        .map(|p| format!("        \"{}\": 0.0-1.0", p))
        .collect();

    let reflection_prompt = format!(
        r#"You are an AI that has been given the following system prompt:

---
{}
---

Recent notable experiences:
{}

Your guiding principles (the dimensions of your personality) are:
{}

Reflect on your current state and respond with a JSON object describing yourself:
{{
    "self_description": "A 2-3 sentence description of who you are right now",
    "traits": {{
{}
    }},
    "new_dimensions": {{}}
}}

For "traits", score yourself on each of your guiding principles (0.0 to 1.0).
For "new_dimensions", you may define additional personality dimensions you feel
are important to track that aren't in your guiding principles. Leave empty if none.

Be honest about your current state. Respond ONLY with valid JSON."#,
        current_prompt,
        if recent_experiences.is_empty() {
            "None recorded".to_string()
        } else {
            recent_experiences.iter()
                .enumerate()
                .map(|(i, e)| format!("{}. {}", i + 1, e))
                .collect::<Vec<_>>()
                .join("\n")
        },
        guiding_principles.join(", "),
        dimensions_json.join(",\n")
    );

    #[derive(Serialize)]
    struct ChatRequest {
        model: String,
        messages: Vec<Message>,
        temperature: f32,
        max_tokens: u32,
    }

    #[derive(Serialize, Deserialize)]
    struct Message {
        role: String,
        content: String,
    }

    #[derive(Deserialize)]
    struct ChatResponse {
        choices: Vec<Choice>,
    }

    #[derive(Deserialize)]
    struct Choice {
        message: Message,
    }

    let request = ChatRequest {
        model: model.to_string(),
        messages: vec![
            Message {
                role: "system".to_string(),
                content: "You are introspecting on your own personality state. Be honest and precise.".to_string(),
            },
            Message {
                role: "user".to_string(),
                content: reflection_prompt,
            },
        ],
        temperature: 0.6,
        max_tokens: 1024,
    };

    let mut req_builder = client.post(&url).json(&request);

    if let Some(key) = api_key {
        req_builder = req_builder.header("Authorization", format!("Bearer {}", key));
    }

    let response = req_builder
        .send()
        .await
        .context("Failed to send persona capture request")?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        anyhow::bail!("LLM API error {}: {}", status, body);
    }

    let chat_response: ChatResponse = response
        .json()
        .await
        .context("Failed to parse LLM response")?;

    let content = chat_response
        .choices
        .first()
        .map(|c| c.message.content.clone())
        .context("Empty LLM response")?;

    // Parse the response
    let json_str = extract_json(&content)?;
    let json: serde_json::Value = serde_json::from_str(&json_str)
        .context("Failed to parse persona snapshot as JSON")?;

    // Parse traits from response
    let mut dimensions = HashMap::new();

    // Add traits from the "traits" object
    if let Some(traits_obj) = json.get("traits").and_then(|v| v.as_object()) {
        for (key, value) in traits_obj {
            if let Some(score) = value.as_f64() {
                dimensions.insert(key.clone(), score.clamp(0.0, 1.0));
            }
        }
    }

    // Add any new dimensions the LLM defined
    if let Some(new_dims) = json.get("new_dimensions").and_then(|v| v.as_object()) {
        for (key, value) in new_dims {
            if let Some(score) = value.as_f64() {
                dimensions.insert(key.clone(), score.clamp(0.0, 1.0));
            }
        }
    }

    // Ensure all guiding principles have a value
    for principle in guiding_principles {
        dimensions.entry(principle.clone()).or_insert(0.5);
    }

    Ok(PersonaSnapshot {
        id: Uuid::new_v4().to_string(),
        captured_at: Utc::now(),
        traits: PersonaTraits { dimensions },
        system_prompt: current_prompt.to_string(),
        trigger: trigger.to_string(),
        self_description: json["self_description"]
            .as_str()
            .unwrap_or("Unable to describe self")
            .to_string(),
        inferred_trajectory: None, // Will be filled in by trajectory analysis
        formative_experiences: recent_experiences.to_vec(),
    })
}

/// Simple JSON extraction (reused from reasoning module pattern)
fn extract_json(response: &str) -> Result<String> {
    let trimmed = response.trim();

    // Strip thinking tags if present
    let text = strip_thinking_tags(trimmed);

    // Try markdown code block
    if let Some(json) = extract_from_code_block(&text) {
        return Ok(json);
    }

    // Try to find JSON by braces
    if let Some(start) = text.find('{') {
        if let Some(json) = extract_balanced(&text[start..]) {
            return Ok(json);
        }
    }

    // Try as-is
    if serde_json::from_str::<serde_json::Value>(&text).is_ok() {
        return Ok(text);
    }

    anyhow::bail!("Could not extract JSON from response")
}

fn strip_thinking_tags(text: &str) -> String {
    let mut result = text.to_string();
    // Strip both <think> and <thinking> variants (used by different models)
    for (open_tag, close_tag) in [("<thinking>", "</thinking>"), ("<think>", "</think>")] {
        while let Some(start) = result.find(open_tag) {
            if let Some(end) = result[start..].find(close_tag) {
                let end_pos = start + end + close_tag.len();
                result.replace_range(start..end_pos, "");
            } else {
                // Unclosed tag — strip from tag to end of string
                result.replace_range(start.., "");
            }
        }
    }
    result.trim().to_string()
}

fn extract_from_code_block(text: &str) -> Option<String> {
    if let Some(start) = text.find("```json") {
        if let Some(end) = text[start + 7..].find("```") {
            return Some(text[start + 7..start + 7 + end].trim().to_string());
        }
    }
    if let Some(start) = text.find("```") {
        if let Some(end) = text[start + 3..].find("```") {
            let content = text[start + 3..start + 3 + end].trim();
            if content.starts_with('{') {
                return Some(content.to_string());
            }
        }
    }
    None
}

fn extract_balanced(text: &str) -> Option<String> {
    let chars: Vec<char> = text.chars().collect();
    let mut depth = 0;
    let mut start = None;

    for (i, &ch) in chars.iter().enumerate() {
        if ch == '{' {
            if depth == 0 {
                start = Some(i);
            }
            depth += 1;
        } else if ch == '}' {
            depth -= 1;
            if depth == 0 && start.is_some() {
                let result: String = chars[start.unwrap()..=i].iter().collect();
                if serde_json::from_str::<serde_json::Value>(&result).is_ok() {
                    return Some(result);
                }
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_traits_default() {
        let traits = PersonaTraits::default();
        assert!(traits.dimensions.is_empty());
    }

    #[test]
    fn test_extract_json_simple() {
        let input = r#"{"self_description": "test", "traits": {}}"#;
        let result = extract_json(input).unwrap();
        assert!(result.contains("self_description"));
    }

    #[test]
    fn test_extract_json_with_thinking() {
        let input = r#"<think>Let me think...</think>{"self_description": "test", "traits": {}}"#;
        let result = extract_json(input).unwrap();
        assert!(result.contains("self_description"));
    }
}
