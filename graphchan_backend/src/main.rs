use anyhow::Result;
use graphchan_backend::bootstrap;
use graphchan_backend::cli;
use graphchan_backend::config::GraphchanConfig;
use graphchan_backend::network;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let config = GraphchanConfig::from_env()?;
    let bootstrap = bootstrap::initialize(&config).await?;
    let network =
        network::NetworkHandle::start(&config.paths, &config.network, bootstrap.database.clone())
            .await?;
    let identity = bootstrap.identity.clone();
    tracing::info!(
        directories_created = ?bootstrap.directories_created,
        database_initialized = bootstrap.database_initialized,
        gpg_fingerprint = %bootstrap.identity.gpg_fingerprint,
        iroh_peer_id = %bootstrap.identity.iroh_peer_id,
        "bootstrap complete"
    );

    cli::run(config, identity, bootstrap.database, network).await
}

fn init_tracing() {
    let env_filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "graphchan_backend=info,tower_http=info".into());
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}
