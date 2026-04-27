use anyhow::{anyhow, Result};
use graphchan_backend::api;
use graphchan_backend::config::GraphchanConfig;
use graphchan_backend::node::GraphchanNode;
use graphchan_backend::telemetry;
use graphchan_backend::utils;
use tokio::runtime::Runtime;
use tracing::error;

fn main() -> Result<()> {
    utils::print_banner();
    telemetry::init_tracing();

    let runtime = Runtime::new()?;
    let mut config = GraphchanConfig::from_env()?;

    // If the user hasn't set GRAPHCHAN_API_TOKEN, mint a per-launch random token
    // so the bundled desktop API isn't an open door for anything else on the
    // host. The frontend reads it via GRAPHCHAN_API_TOKEN below.
    if config.auth.token.is_none() {
        let token: String = (0..32)
            .map(|_| {
                const CHARSET: &[u8] =
                    b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
                let idx = rand::random::<u8>() as usize % CHARSET.len();
                CHARSET[idx] as char
            })
            .collect();
        config.auth.token = Some(token);
    }
    let api_token = config.auth.token.clone().expect("token set above");

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
    std::env::set_var("GRAPHCHAN_API_TOKEN", &api_token);

    let ui_result = graphchan_frontend::run_frontend();

    server.abort();
    let _ = runtime.block_on(async {
        snapshot.network.shutdown().await;
    });

    ui_result.map_err(|err| anyhow!(err.to_string()))
}
