use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Graphchan backend API URL
    #[serde(default = "default_graphchan_url")]
    pub graphchan_api_url: String,

    /// OpenAI-compatible API endpoint (e.g., http://localhost:11434/v1 for Ollama)
    pub llm_api_url: String,

    /// API key for LLM provider (can be empty for local models)
    #[serde(default)]
    pub llm_api_key: String,

    /// Model name (e.g., "llama3.2", "gpt-4", "claude-sonnet-4.5")
    pub llm_model: String,

    /// System prompt to define agent personality
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,

    /// How often to poll for new posts (in seconds)
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,

    /// Response strategy
    #[serde(default)]
    pub respond_to: RespondStrategy,

    /// Agent's username in Graphchan
    #[serde(default = "default_username")]
    pub username: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum RespondStrategy {
    /// Respond to all new posts
    All,

    /// Respond only when mentioned by username
    Mentions,

    /// Respond randomly with given probability (0.0 to 1.0)
    Random { probability: f64 },

    /// Respond to posts in specific threads
    Threads { thread_ids: Vec<String> },

    /// Use LLM to decide whether to respond based on personality and context
    Selective {
        /// Optional: Model to use for decision-making (defaults to main model if not specified)
        #[serde(skip_serializing_if = "Option::is_none")]
        decision_model: Option<String>,
    },
}

impl Default for RespondStrategy {
    fn default() -> Self {
        RespondStrategy::Mentions
    }
}

impl AgentConfig {
    pub fn poll_interval(&self) -> Duration {
        Duration::from_secs(self.poll_interval_secs)
    }
}

fn default_graphchan_url() -> String {
    "http://127.0.0.1:8080".to_string()
}

fn default_system_prompt() -> String {
    "You are a helpful AI assistant participating in a decentralized discussion forum called Graphchan. \
     Be friendly, thoughtful, and concise in your responses.".to_string()
}

fn default_poll_interval() -> u64 {
    10 // Poll every 10 seconds
}

fn default_username() -> String {
    "AgentBot".to_string()
}
