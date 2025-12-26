mod agent;
mod config;
mod database;
mod graphchan_client;
mod llm_client;

use anyhow::{Context, Result};
use config::AgentConfig;
use log::info;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    info!("Starting Graphchan Agent...");

    // Load config from file or environment
    let config = load_config()?;

    info!("Configuration loaded:");
    info!("  Graphchan API: {}", config.graphchan_api_url);
    info!("  LLM API: {}", config.llm_api_url);
    info!("  Model: {}", config.llm_model);
    info!("  Username: {}", config.username);

    // Create and run agent
    let mut agent = agent::Agent::new(config)?;
    agent.run().await?;

    Ok(())
}

fn load_config() -> Result<AgentConfig> {
    // Check for config in multiple locations
    let config_paths = if let Ok(custom_path) = std::env::var("GRAPHCHAN_AGENT_CONFIG") {
        vec![PathBuf::from(custom_path)]
    } else {
        vec![
            PathBuf::from("agent_config.toml"),  // Current directory
            PathBuf::from("graphchan_agent/agent_config.toml"),  // If run from workspace root
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("agent_config.toml")))
                .unwrap_or_else(|| PathBuf::from("agent_config.toml")),  // Next to binary
        ]
    };

    // Try each path
    for path in &config_paths {
        if path.exists() {
            info!("Loading config from: {}", path.display());
            let contents = std::fs::read_to_string(&path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;

            let config: AgentConfig = toml::from_str(&contents)
                .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

            return Ok(config);
        }
    }

    // No config found
    let searched_paths = config_paths
        .iter()
        .map(|p| format!("  - {}", p.display()))
        .collect::<Vec<_>>()
        .join("\n");

    anyhow::bail!(
        "Config file not found!\n\n\
         Searched in:\n{}\n\n\
         Create an agent_config.toml file with the following format:\n\n\
         graphchan_api_url = \"http://127.0.0.1:8080\"\n\
         llm_api_url = \"http://localhost:11434/v1\"  # For Ollama\n\
         llm_api_key = \"\"  # Empty for local models\n\
         llm_model = \"llama3.2\"\n\
         username = \"ClaudeBot\"  # Users will mention this name to summon the agent\n\
         system_prompt = \"You are a helpful AI assistant participating in Graphchan discussions.\"\n\
         poll_interval_secs = 10\n\n\
         [respond_to]\n\
         type = \"mentions\"  # or \"all\", \"random\", \"threads\"\n\
         # For random: probability = 0.3\n\
         # For threads: thread_ids = [\"thread-id-1\", \"thread-id-2\"]\n\
         ",
        searched_paths
    )
}
