use anyhow::Result;
use clap::{Parser, Subcommand};
use graphchan_backend::cli;
use graphchan_backend::config::GraphchanConfig;
use graphchan_backend::node::GraphchanNode;
use graphchan_backend::telemetry;
use graphchan_backend::utils;

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
    utils::print_banner();
    telemetry::init_tracing();

    let args = Args::parse();

    let config = GraphchanConfig::from_env()?;
    let node = GraphchanNode::start(config).await?;
    tracing::info!(
        gpg_fingerprint = %node.identity().gpg_fingerprint,
        iroh_peer_id = %node.identity().iroh_peer_id,
        "bootstrap complete"
    );

    match args.command.unwrap_or(Command::Cli) {
        Command::Serve => node.run_http_server().await,
        Command::Cli => {
            let snapshot = node.snapshot();
            cli::run_cli(
                snapshot.config,
                snapshot.identity,
                snapshot.database,
                snapshot.network,
                snapshot.blobs,
            )
            .await
        }
    }
}
