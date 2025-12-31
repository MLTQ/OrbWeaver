mod agent;
mod api_client;
mod character_card;
mod config;
mod models;
mod ui;

use std::sync::Arc;
use tracing_subscriber::EnvFilter;

use agent::Agent;
use api_client::GraphchanClient;
use config::AgentConfig;
use ui::app::AgentApp;

fn main() {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,graphchan_agent=debug"))
        )
        .init();

    tracing::info!("🤖 Graphchan Agent starting...");

    // Load configuration from file (falls back to env vars if no file exists)
    let config = AgentConfig::load();

    tracing::info!("📡 Connecting to Graphchan at {}", config.graphchan_api_url);
    tracing::info!("🧠 LLM: {} at {}", config.llm_model, config.llm_api_url);

    // Check if LLM is reachable (non-fatal)
    tracing::info!("💡 Tip: Make sure your LLM is running (e.g., `ollama serve` for Ollama)");

    // Create API client
    let client = GraphchanClient::new(config.graphchan_api_url.clone());

    // Create event channel
    let (event_tx, event_rx) = flume::unbounded();

    // Create agent
    let agent = Arc::new(Agent::new(client, config.clone(), event_tx));

    // Spawn agent loop in background
    let agent_clone = agent.clone();
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            if let Err(e) = agent_clone.run_loop().await {
                tracing::error!("Agent loop error: {}", e);
            }
        });
    });

    // Launch UI
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([600.0, 800.0])
            .with_title("Graphchan Agent"),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Graphchan Agent",
        native_options,
        Box::new(|_cc| Ok(Box::new(AgentApp::new(event_rx, agent, config)))),
    ) {
        tracing::error!("UI error: {}", e);
        std::process::exit(1);
    }
}
