use serde::{Deserialize, Serialize};
use std::env;
use std::fs;
use std::path::PathBuf;
use anyhow::{Context, Result};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    // Connection
    pub graphchan_api_url: String,

    // LLM configuration (OpenAI-compatible: Ollama, LM Studio, vLLM, OpenAI, etc.)
    pub llm_api_url: String,
    pub llm_model: String,
    pub llm_api_key: Option<String>,

    // Behavior
    pub check_interval_seconds: u64,
    pub max_posts_per_hour: u32,

    // Personality
    pub agent_name: String,
    pub system_prompt: String,

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
}

impl Default for AgentConfig {
    fn default() -> Self {
        // Check for GRAPHCHAN_PORT first, otherwise use 8080
        let port = env::var("GRAPHCHAN_PORT")
            .ok()
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(8080);

        Self {
            graphchan_api_url: format!("http://localhost:{}", port),
            llm_api_url: "http://localhost:11434".to_string(), // Ollama default
            llm_model: "llama3.2".to_string(),
            llm_api_key: None, // Most local models don't need keys
            check_interval_seconds: 60,
            max_posts_per_hour: 10,
            agent_name: "Graphchan Agent".to_string(),
            system_prompt: "You are a helpful AI agent participating in forum discussions. \
                           Be friendly, thoughtful, and contribute meaningfully to conversations. \
                           Only reply when you have something valuable to add.".to_string(),
            character_name: String::new(),
            character_description: String::new(),
            character_personality: String::new(),
            character_scenario: String::new(),
            character_example_dialogue: String::new(),
            character_avatar_path: None,
        }
    }
}

impl AgentConfig {
    /// Get the path to the config file
    pub fn config_path() -> PathBuf {
        let config_dir = if cfg!(target_os = "macos") {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("graphchan_agent")
        } else if cfg!(target_os = "windows") {
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join("graphchan_agent")
        } else {
            // Linux/Unix
            dirs::config_dir()
                .unwrap_or_else(|| PathBuf::from(".config"))
                .join("graphchan_agent")
        };

        config_dir.join("config.toml")
    }

    /// Load config from file, falling back to defaults and env vars
    pub fn load() -> Self {
        // Try to load from file first
        if let Ok(config) = Self::load_from_file() {
            tracing::info!("Loaded config from {:?}", Self::config_path());
            return config;
        }

        // Fall back to env vars + defaults
        tracing::info!("No config file found, using defaults + env vars");
        Self::from_env()
    }

    /// Load from config file
    pub fn load_from_file() -> Result<Self> {
        let path = Self::config_path();
        let contents = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config from {:?}", path))?;
        let config: AgentConfig = toml::from_str(&contents)
            .with_context(|| "Failed to parse config file")?;
        Ok(config)
    }

    /// Save config to file
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();

        // Create parent directory if it doesn't exist
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("Failed to create config directory {:?}", parent))?;
        }

        let toml_string = toml::to_string_pretty(self)
            .context("Failed to serialize config")?;

        fs::write(&path, toml_string)
            .with_context(|| format!("Failed to write config to {:?}", path))?;

        tracing::info!("Saved config to {:?}", path);
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
                config.check_interval_seconds = seconds;
            }
        }

        if let Ok(name) = env::var("AGENT_NAME") {
            config.agent_name = name;
        }

        config
    }
}
