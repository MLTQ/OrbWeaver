use anyhow::{anyhow, Result};
use graphchan_backend::api;
use graphchan_backend::config::GraphchanConfig;
use graphchan_backend::node::GraphchanNode;
use graphchan_backend::telemetry;
use tokio::runtime::Runtime;
use tracing::error;

fn main() -> Result<()> {
    telemetry::init_tracing();

    let runtime = Runtime::new()?;
    let config = GraphchanConfig::from_env()?;
    let node = runtime.block_on(GraphchanNode::start(config))?;
    let snapshot = node.snapshot();
    drop(node);

    let server_snapshot = snapshot.clone();
    let server = runtime.spawn(async move {
        if let Err(err) = api::serve_http(
            server_snapshot.config,
            server_snapshot.identity,
            server_snapshot.database,
            server_snapshot.network,
            server_snapshot.blobs,
        )
        .await
        {
            error!(error = ?err, "embedded HTTP server exited");
        }
    });

    let base_url = format!("http://127.0.0.1:{}", snapshot.config.api_port);
    std::env::set_var("GRAPHCHAN_API_URL", &base_url);

    let ui_result = graphchan_frontend::run_frontend();

    server.abort();
    let _ = runtime.block_on(async {
        snapshot.network.shutdown().await;
    });

    ui_result.map_err(|err| anyhow!(err.to_string()))
}
