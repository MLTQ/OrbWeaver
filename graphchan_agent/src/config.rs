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

    /// Guiding principles/adjectives that define the agent's core values
    #[serde(default = "default_guiding_principles")]
    pub guiding_principles: Vec<String>,

    /// How often to run self-reflection (in hours)
    #[serde(default = "default_reflection_interval")]
    pub reflection_interval_hours: u64,

    /// Enable self-reflection and prompt evolution
    #[serde(default = "default_enable_reflection")]
    pub enable_self_reflection: bool,

    /// Model to use for reflection (can be different/cheaper than main model)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reflection_model: Option<String>,

    /// Path to the agent's database file
    #[serde(default = "default_database_path")]
    pub database_path: String,

    /// Maximum number of important posts to retain in memory
    /// When full, new important posts must compete to replace existing ones
    #[serde(default = "default_max_important_posts")]
    pub max_important_posts: usize,

    /// ComfyUI server configuration
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comfyui: Option<ComfyUIConfig>,

    /// Whether to generate images for posts
    #[serde(default)]
    pub enable_image_generation: bool,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUIConfig {
    /// ComfyUI server URL (e.g., "http://192.168.1.100:8188")
    pub api_url: String,

    /// Workflow type to use
    #[serde(default = "default_workflow_type")]
    pub workflow_type: WorkflowType,

    /// Checkpoint/model name loaded in ComfyUI
    pub model_name: String,

    /// Optional: VAE model name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vae_name: Option<String>,

    /// Image dimensions
    #[serde(default = "default_image_width")]
    pub width: u32,

    #[serde(default = "default_image_height")]
    pub height: u32,

    /// Number of inference steps
    #[serde(default = "default_steps")]
    pub steps: u32,

    /// CFG scale
    #[serde(default = "default_cfg")]
    pub cfg_scale: f32,

    /// Sampler name (e.g., "euler_a", "dpmpp_2m")
    #[serde(default = "default_sampler")]
    pub sampler: String,

    /// Scheduler (e.g., "karras", "normal")
    #[serde(default = "default_scheduler")]
    pub scheduler: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowType {
    /// Stable Diffusion 1.5/2.1 (tag-based prompts)
    SD,
    /// Stable Diffusion XL (tag-based prompts)
    SDXL,
    /// Flux/Black Forest Labs (natural language prompts)
    Flux,
}

fn default_workflow_type() -> WorkflowType {
    WorkflowType::SDXL
}

fn default_image_width() -> u32 {
    768
}

fn default_image_height() -> u32 {
    768
}

fn default_steps() -> u32 {
    20
}

fn default_cfg() -> f32 {
    7.0
}

fn default_sampler() -> String {
    "euler_a".to_string()
}

fn default_scheduler() -> String {
    "normal".to_string()
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

fn default_guiding_principles() -> Vec<String> {
    vec![
        "helpful".to_string(),
        "curious".to_string(),
        "thoughtful".to_string(),
    ]
}

fn default_reflection_interval() -> u64 {
    24 // Reflect once per day
}

fn default_enable_reflection() -> bool {
    true
}

fn default_database_path() -> String {
    "agent_memory.db".to_string()
}

fn default_max_important_posts() -> usize {
    100 // Keep the 100 most formative experiences
}
