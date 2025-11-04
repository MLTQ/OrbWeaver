use crate::api;
use crate::config::GraphchanConfig;
use crate::database::Database;
use crate::identity::IdentitySummary;
use crate::network::NetworkHandle;
use anyhow::Result;

/// Runs the primary CLI command. For now we only start the HTTP/API server.
pub async fn run(
    config: GraphchanConfig,
    identity: IdentitySummary,
    database: Database,
    network: NetworkHandle,
) -> Result<()> {
    tracing::info!(port = config.api_port, "starting Graphchan backend");
    api::serve_http(config, identity, database, network).await
}
