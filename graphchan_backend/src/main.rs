use anyhow::Result;
use clap::{Parser, Subcommand};
use graphchan_backend::bootstrap;
use graphchan_backend::cli;
use graphchan_backend::config::GraphchanConfig;
use graphchan_backend::network;
use iroh_blobs::store::fs::FsStore;

#[derive(Parser)]
#[command(author, version, about = "Graphchan backend daemon and CLI")]
struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Run the HTTP server (Axum) for REST/API access
    Serve,
    /// Start the interactive CLI for friendcodes, threads, and posts
    Cli,
}

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let args = Args::parse();

    let config = GraphchanConfig::from_env()?;
    let bootstrap = bootstrap::initialize(&config).await?;
    let blob_store = FsStore::load(&config.paths.blobs_dir).await?;
    let network = network::NetworkHandle::start(
        &config.paths,
        &config.network,
        blob_store.clone(),
        bootstrap.database.clone(),
    )
    .await?;
    tracing::info!(
        directories_created = ?bootstrap.directories_created,
        database_initialized = bootstrap.database_initialized,
        gpg_fingerprint = %bootstrap.identity.gpg_fingerprint,
        iroh_peer_id = %bootstrap.identity.iroh_peer_id,
        "bootstrap complete"
    );

    match args.command.unwrap_or(Command::Cli) {
        Command::Serve => {
            cli::run_server(
                config,
                bootstrap.identity.clone(),
                bootstrap.database.clone(),
                network,
                blob_store.clone(),
            )
            .await
        }
        Command::Cli => {
            cli::run_cli(
                config,
                bootstrap.identity.clone(),
                bootstrap.database.clone(),
                network,
                blob_store,
            )
            .await
        }
    }
}

fn init_tracing() {
    let env_filter = std::env::var("RUST_LOG")
        .unwrap_or_else(|_| "graphchan_backend=info,tower_http=info".into());
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_env_filter(env_filter)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);
}
