mod agent;
mod character_card;
mod comfyui_client;
mod config;
mod database;
mod graphchan_client;
mod llm_client;

use anyhow::{Context, Result};
use chrono::Utc;
use clap::{Parser, Subcommand};
use config::AgentConfig;
use log::info;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "graphchan_agent")]
#[command(about = "AI agent for Graphchan decentralized forum", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run the agent (default)
    Run,

    /// Import a character card from a file
    ImportCharacter {
        /// Path to the character card file (JSON, W++, or Boostyle format)
        #[arg(short, long)]
        file: PathBuf,
    },

    /// Reset to the original character (removes imported character card)
    ResetCharacter,

    /// Show current character information
    ShowCharacter,
}

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::init();

    let cli = Cli::parse();

    // Load config from file or environment
    let config = load_config()?;

    match cli.command {
        Some(Commands::ImportCharacter { file }) => {
            import_character(&config, file)?;
        }
        Some(Commands::ResetCharacter) => {
            reset_character(&config)?;
        }
        Some(Commands::ShowCharacter) => {
            show_character(&config)?;
        }
        Some(Commands::Run) | None => {
            // Default: run the agent
            info!("Starting Graphchan Agent...");
            info!("Configuration loaded:");
            info!("  Graphchan API: {}", config.graphchan_api_url);
            info!("  LLM API: {}", config.llm_api_url);
            info!("  Model: {}", config.llm_model);
            info!("  Username: {}", config.username);

            let mut agent = agent::Agent::new(config)?;
            agent.run().await?;
        }
    }

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

fn import_character(config: &AgentConfig, file_path: PathBuf) -> Result<()> {
    info!("Importing character from: {}", file_path.display());

    // Parse the character card
    let (parsed_character, format, original_data) = character_card::parse_character_card(&file_path)
        .context("Failed to parse character card")?;

    info!("Successfully parsed {} format", format);
    info!("Character name: {}", parsed_character.name);

    // Convert to system prompt
    let derived_prompt = character_card::character_to_system_prompt(&parsed_character);

    info!("Generated system prompt:\n{}\n", derived_prompt);

    // Save to database
    let db = database::AgentDatabase::new(&config.database_path)?;

    let character_card = database::CharacterCard {
        id: uuid::Uuid::new_v4().to_string(),
        imported_at: Utc::now(),
        format,
        original_data,
        derived_prompt: derived_prompt.clone(),
        name: Some(parsed_character.name.clone()),
        description: Some(parsed_character.description.clone()),
    };

    db.save_character_card(&character_card)?;

    // Set as current system prompt
    db.set_current_system_prompt(&derived_prompt)?;

    info!("✓ Character card imported successfully!");
    info!("✓ System prompt updated");
    info!("\nThe agent will now use this character when it restarts.");

    Ok(())
}

fn reset_character(config: &AgentConfig) -> Result<()> {
    info!("Resetting to default character...");

    let db = database::AgentDatabase::new(&config.database_path)?;

    // Delete character card
    db.delete_character_card()?;

    // Reset to default system prompt from config
    db.set_current_system_prompt(&config.system_prompt)?;

    info!("✓ Character card removed");
    info!("✓ Reset to default system prompt");
    info!("\nThe agent will use the default prompt from config when it restarts.");

    Ok(())
}

fn show_character(config: &AgentConfig) -> Result<()> {
    let db = database::AgentDatabase::new(&config.database_path)?;

    if let Some(card) = db.get_character_card()? {
        info!("Current Character Card:");
        info!("  Name: {}", card.name.as_deref().unwrap_or("(none)"));
        info!("  Format: {}", card.format);
        info!("  Imported: {}", card.imported_at.format("%Y-%m-%d %H:%M:%S"));
        info!("\nCurrent System Prompt:");
        info!("{}", card.derived_prompt);
    } else {
        info!("No character card imported.");
        info!("\nUsing default system prompt from config:");
        if let Some(current_prompt) = db.get_current_system_prompt()? {
            info!("{}", current_prompt);
        } else {
            info!("{}", config.system_prompt);
        }
    }

    Ok(())
}
