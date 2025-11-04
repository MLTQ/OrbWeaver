use crate::config::GraphchanConfig;
use crate::database::Database;
use crate::identity::{ensure_local_identity, IdentitySummary};
use anyhow::Result;
use std::fs;

pub struct BootstrapResources {
    pub directories_created: Vec<String>,
    pub database_initialized: bool,
    pub identity: IdentitySummary,
    pub database: Database,
}

pub async fn initialize(config: &GraphchanConfig) -> Result<BootstrapResources> {
    let mut directories_created = Vec::new();
    create_dir_if_missing(&config.paths.data_dir, &mut directories_created)?;
    create_dir_if_missing(&config.paths.files_dir, &mut directories_created)?;
    create_dir_if_missing(&config.paths.uploads_dir, &mut directories_created)?;
    create_dir_if_missing(&config.paths.downloads_dir, &mut directories_created)?;
    create_dir_if_missing(&config.paths.keys_dir, &mut directories_created)?;
    create_dir_if_missing(&config.paths.gpg_dir, &mut directories_created)?;
    create_dir_if_missing(&config.paths.logs_dir, &mut directories_created)?;

    let database = Database::connect(&config.paths)?;
    let database_initialized = database.ensure_migrations()?;

    let identity = ensure_local_identity(&config.paths)?;
    database.save_identity(
        &identity.gpg_fingerprint,
        &identity.iroh_peer_id,
        &identity.friendcode,
    )?;
    database.upsert_local_peer(
        &identity.gpg_fingerprint,
        &identity.iroh_peer_id,
        &identity.friendcode,
    )?;

    Ok(BootstrapResources {
        directories_created,
        database_initialized,
        identity,
        database,
    })
}

fn create_dir_if_missing(path: &std::path::Path, created: &mut Vec<String>) -> Result<()> {
    if !path.exists() {
        fs::create_dir_all(path)?;
        created.push(path.display().to_string());
    }
    Ok(())
}
