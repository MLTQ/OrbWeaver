use crate::api;
use crate::bootstrap::{self, BootstrapResources};
use crate::config::GraphchanConfig;
use crate::database::Database;
use crate::identity::IdentitySummary;
use crate::network::NetworkHandle;
use anyhow::Result;
use iroh_blobs::store::fs::FsStore;

/// Convenience wrapper that bootstraps the backend once and hands out
/// cloned handles for whichever entrypoint (CLI, REST server, embedded UI)
/// needs them.
pub struct GraphchanNode {
    config: GraphchanConfig,
    bootstrap: BootstrapResources,
    blob_store: FsStore,
    network: NetworkHandle,
}

impl GraphchanNode {
    /// Bootstraps all persistent state, loads the blob store, and starts the
    /// networking stack.
    pub async fn start(config: GraphchanConfig) -> Result<Self> {
        let bootstrap = bootstrap::initialize(&config).await?;
        let blob_store = FsStore::load(&config.paths.blobs_dir).await?;
        let network = NetworkHandle::start(
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
            "graphchan node initialized"
        );

        Ok(Self {
            config,
            bootstrap,
            blob_store,
            network,
        })
    }

    /// Returns a snapshot of the node's reusable handles.
    pub fn snapshot(&self) -> NodeSnapshot {
        NodeSnapshot {
            config: self.config.clone(),
            identity: self.bootstrap.identity.clone(),
            database: self.bootstrap.database.clone(),
            network: self.network.clone(),
            blobs: self.blob_store.clone(),
        }
    }

    /// Runs the REST API server until shutdown.
    pub async fn run_http_server(&self) -> Result<()> {
        let snapshot = self.snapshot();
        api::serve_http(
            snapshot.config,
            snapshot.identity,
            snapshot.database,
            snapshot.network,
            snapshot.blobs,
        )
        .await
    }

    /// Returns the local identity details.
    pub fn identity(&self) -> &IdentitySummary {
        &self.bootstrap.identity
    }

    /// Returns a clone of the database handle.
    pub fn database(&self) -> Database {
        self.bootstrap.database.clone()
    }

    /// Returns the running network handle.
    pub fn network(&self) -> NetworkHandle {
        self.network.clone()
    }

    /// Returns the loaded blob store.
    pub fn blobs(&self) -> FsStore {
        self.blob_store.clone()
    }
}

/// Cloned handles suitable for consumers that just need read/write access to
/// backend services without owning the entire node struct.
#[derive(Clone)]
pub struct NodeSnapshot {
    pub config: GraphchanConfig,
    pub identity: IdentitySummary,
    pub database: Database,
    pub network: NetworkHandle,
    pub blobs: FsStore,
}
