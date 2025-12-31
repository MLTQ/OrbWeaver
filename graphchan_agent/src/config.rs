use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RespondTo {
    #[serde(rename = "type")]
    pub response_type: String,
    #[serde(default)]
    pub decision_model: Option<String>,
}

impl Default for RespondTo {
    fn default() -> Self {
        Self {
            response_type: "selective".to_string(),
            decision_model: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComfyUIConfig {
    #[serde(default = "default_comfyui_url")]
    pub api_url: String,
    #[serde(default = "default_workflow_type")]
    pub workflow_type: String,
    #[serde(default = "default_model_name")]
    pub model_name: String,
    #[serde(default)]
    pub vae_name: Option<String>,
    #[serde(default = "default_width")]
    pub width: u32,
    #[serde(default = "default_height")]
    pub height: u32,
    #[serde(default = "default_steps")]
    pub steps: u32,
    #[serde(default = "default_cfg_scale")]
    pub cfg_scale: f32,
    #[serde(default = "default_sampler")]
    pub sampler: String,
    #[serde(default = "default_scheduler")]
    pub scheduler: String,
    #[serde(default)]
    pub negative_prompt: String,
}

fn default_comfyui_url() -> String {
    "http://127.0.0.1:8188".to_string()
}

fn default_workflow_type() -> String {
    "sd".to_string()
}

fn default_model_name() -> String {
    "v1-5-pruned-emaonly.safetensors".to_string()
}

fn default_width() -> u32 {
    512
}

fn default_height() -> u32 {
    512
}

fn default_steps() -> u32 {
    20
}

fn default_cfg_scale() -> f32 {
    7.0
}

fn default_sampler() -> String {
    "euler".to_string()
}

fn default_scheduler() -> String {
    "normal".to_string()
}

impl Default for ComfyUIConfig {
    fn default() -> Self {
        Self {
            api_url: default_comfyui_url(),
            workflow_type: default_workflow_type(),
            model_name: default_model_name(),
            vae_name: None,
            width: default_width(),
            height: default_height(),
            steps: default_steps(),
            cfg_scale: default_cfg_scale(),
            sampler: default_sampler(),
            scheduler: default_scheduler(),
            negative_prompt: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    // Connection
    #[serde(default = "default_graphchan_url")]
    pub graphchan_api_url: String,

    // LLM configuration (OpenAI-compatible: Ollama, LM Studio, vLLM, OpenAI, etc.)
    #[serde(default = "default_llm_url")]
    pub llm_api_url: String,
    #[serde(default = "default_llm_model")]
    pub llm_model: String,
    #[serde(default)]
    pub llm_api_key: Option<String>,

    // Agent Identity
    #[serde(default = "default_username", alias = "agent_name")]
    pub username: String,

    // System prompt
    #[serde(default = "default_system_prompt")]
    pub system_prompt: String,

    // Polling and Response
    #[serde(default = "default_poll_interval", alias = "check_interval_seconds")]
    pub poll_interval_secs: u64,

    #[serde(default)]
    pub respond_to: RespondTo,

    // Self-reflection and evolution
    #[serde(default)]
    pub enable_self_reflection: bool,
    #[serde(default = "default_reflection_interval")]
    pub reflection_interval_hours: u64,
    #[serde(default)]
    pub reflection_model: Option<String>,

    // Guiding principles
    #[serde(default)]
    pub guiding_principles: Vec<String>,

    // Memory and database
    #[serde(default = "default_database_path")]
    pub database_path: String,
    #[serde(default = "default_max_important_posts")]
    pub max_important_posts: u32,

    // Image generation
    #[serde(default)]
    pub enable_image_generation: bool,
    #[serde(default)]
    pub comfyui: ComfyUIConfig,

    // Character Card (optional)
    #[serde(default)]
    pub character_name: String,
    #[serde(default)]
    pub character_description: String,
    #[serde(default)]
    pub character_personality: String,
    #[serde(default)]
    pub character_scenario: String,
    #[serde(default)]
    pub character_example_dialogue: String,
    #[serde(default)]
    pub character_avatar_path: Option<String>,

    // Legacy fields for backward compatibility
    #[serde(default)]
    pub max_posts_per_hour: u32,
}

fn default_graphchan_url() -> String {
    env::var("GRAPHCHAN_PORT")
        .ok()
        .and_then(|p| p.parse::<u16>().ok())
        .map(|port| format!("http://localhost:{}", port))
        .unwrap_or_else(|| "http://localhost:8080".to_string())
}

fn default_llm_url() -> String {
    "http://localhost:11434".to_string()
}

fn default_llm_model() -> String {
    "llama3.2".to_string()
}

fn default_username() -> String {
    "Graphchan Agent".to_string()
}

fn default_system_prompt() -> String {
    "You are a helpful AI agent participating in forum discussions. \
     Be friendly, thoughtful, and contribute meaningfully to conversations. \
     Only reply when you have something valuable to add.".to_string()
}

fn default_poll_interval() -> u64 {
    60
}

fn default_reflection_interval() -> u64 {
    24
}

fn default_database_path() -> String {
    "agent_memory.db".to_string()
}

fn default_max_important_posts() -> u32 {
    100
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            graphchan_api_url: default_graphchan_url(),
            llm_api_url: default_llm_url(),
            llm_model: default_llm_model(),
            llm_api_key: None,
            username: default_username(),
            system_prompt: default_system_prompt(),
            poll_interval_secs: default_poll_interval(),
            respond_to: RespondTo::default(),
            enable_self_reflection: false,
            reflection_interval_hours: default_reflection_interval(),
            reflection_model: None,
            guiding_principles: vec![
                "helpful".to_string(),
                "curious".to_string(),
                "thoughtful".to_string(),
            ],
            database_path: default_database_path(),
            max_important_posts: default_max_important_posts(),
            enable_image_generation: false,
            comfyui: ComfyUIConfig::default(),
            character_name: String::new(),
            character_description: String::new(),
            character_personality: String::new(),
            character_scenario: String::new(),
            character_example_dialogue: String::new(),
            character_avatar_path: None,
            max_posts_per_hour: 10,
        }
    }
}

impl AgentConfig {
    /// Get the directory containing the executable
    fn get_base_dir() -> PathBuf {
        match std::env::current_exe() {
            Ok(exe_path) => {
                exe_path.parent()
                    .map(|p| p.to_path_buf())
                    .unwrap_or_else(|| PathBuf::from("."))
            }
            Err(_) => PathBuf::from("."),
        }
    }

    /// Get the path to the config file (relative to executable)
    pub fn config_path() -> PathBuf {
        Self::get_base_dir().join("agent_config.toml")
    }

    /// Load config from agent_config.toml (next to executable)
    pub fn load() -> Self {
        let path = Self::config_path();

        match fs::read_to_string(&path) {
            Ok(contents) => {
                match toml::from_str::<AgentConfig>(&contents) {
                    Ok(config) => {
                        tracing::info!("✅ Loaded config from {:?}", path);
                        config
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse {:?}: {}", path, e);
                        tracing::warn!("Using defaults + env vars");
                        Self::from_env()
                    }
                }
            }
            Err(_) => {
                tracing::warn!("No {:?} found", path);
                tracing::info!("Using defaults + env vars");
                Self::from_env()
            }
        }
    }

    /// Save config to file (next to executable)
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();

        let toml_string = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;

        fs::write(&path, toml_string)
            .with_context(|| format!("Failed to write config to {:?}", path))?;

        tracing::info!("💾 Saved config to {:?}", path);
        Ok(())
    }

    /// Load from environment variables (legacy support)
    pub fn from_env() -> Self {
        let mut config = Self::default();

        if let Ok(url) = env::var("GRAPHCHAN_API_URL") {
            config.graphchan_api_url = url;
        }

        if let Ok(url) = env::var("LLM_API_URL") {
            config.llm_api_url = url;
        }

        if let Ok(model) = env::var("LLM_MODEL") {
            config.llm_model = model;
        }

        if let Ok(key) = env::var("LLM_API_KEY") {
            config.llm_api_key = Some(key);
        }

        if let Ok(interval) = env::var("AGENT_CHECK_INTERVAL") {
            if let Ok(seconds) = interval.parse() {
                config.poll_interval_secs = seconds;
            }
        }

        if let Ok(name) = env::var("AGENT_NAME") {
            config.username = name;
        }

        config
    }
}
