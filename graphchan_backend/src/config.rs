use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::env;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct GraphchanConfig {
    pub api_port: u16,
    pub paths: GraphchanPaths,
    pub network: NetworkConfig,
    pub file: FileConfig,
    pub auth: AuthConfig,
}

/// Bearer-token auth for the REST API.
///
/// When `token` is `Some`, every non-public route requires the header
/// `Authorization: Bearer <token>`. The token is read from `GRAPHCHAN_API_TOKEN`
/// at startup. If unset *and* the listen address is loopback, the API runs
/// unauthenticated for backwards compatibility (so existing local desktop
/// installs keep working). If unset and bound to a non-loopback address, the
/// server refuses to start — an open API on a public address is almost certainly
/// not what the user wanted.
#[derive(Debug, Clone, Default)]
pub struct AuthConfig {
    pub token: Option<String>,
    /// Comma-separated list of allowed CORS origins. If `None`, the legacy
    /// open CORS (Any) is used. Set via GRAPHCHAN_CORS_ORIGINS.
    pub cors_origins: Option<Vec<String>>,
}

impl AuthConfig {
    pub fn from_env() -> Self {
        let token = env::var("GRAPHCHAN_API_TOKEN").ok().and_then(|raw| {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
        let cors_origins = env::var("GRAPHCHAN_CORS_ORIGINS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect::<Vec<_>>()
            })
            .filter(|v: &Vec<String>| !v.is_empty());
        Self {
            token,
            cors_origins,
        }
    }
}

impl GraphchanConfig {
    pub fn from_env() -> Result<Self> {
        let paths = GraphchanPaths::discover()?;
        let api_port = env::var("GRAPHCHAN_API_PORT")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(8080);
        let network = NetworkConfig::from_env();
        let file = FileConfig::from_env();
        let auth = AuthConfig::from_env();
        Ok(Self {
            api_port,
            paths,
            network,
            file,
            auth,
        })
    }

    pub fn new(api_port: u16, paths: GraphchanPaths, network: NetworkConfig) -> Self {
        Self {
            api_port,
            paths,
            network,
            file: FileConfig::from_env(),
            auth: AuthConfig::from_env(),
        }
    }

    pub fn with_file(
        api_port: u16,
        paths: GraphchanPaths,
        network: NetworkConfig,
        file: FileConfig,
    ) -> Self {
        Self {
            api_port,
            paths,
            network,
            file,
            auth: AuthConfig::from_env(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub relay_url: Option<String>,
    pub public_addresses: Vec<String>,
    pub enable_dht: bool,
    pub enable_mdns: bool,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            relay_url: None,
            public_addresses: Vec::new(),
            enable_dht: true,  // DHT enabled by default
            enable_mdns: true, // mDNS enabled by default
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct FileConfig {
    pub max_upload_bytes: Option<u64>,
}

impl FileConfig {
    pub fn from_env() -> Self {
        let max_upload_bytes = env::var("GRAPHCHAN_MAX_UPLOAD_BYTES")
            .ok()
            .and_then(|raw| raw.parse::<u64>().ok());
        Self { max_upload_bytes }
    }
}

impl NetworkConfig {
    pub fn from_env() -> Self {
        let relay_url = env::var("GRAPHCHAN_RELAY_URL").ok().and_then(|raw| {
            if raw.trim().is_empty() {
                None
            } else {
                Some(raw)
            }
        });
        let public_addresses = env::var("GRAPHCHAN_PUBLIC_ADDRS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|part| part.trim().to_string())
                    .filter(|part| !part.is_empty())
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        // DHT enabled by default unless explicitly disabled
        let enable_dht = env::var("GRAPHCHAN_DISABLE_DHT")
            .ok()
            .map(|v| v != "1" && v.to_lowercase() != "true")
            .unwrap_or(true);

        // mDNS enabled by default unless explicitly disabled
        let enable_mdns = env::var("GRAPHCHAN_DISABLE_MDNS")
            .ok()
            .map(|v| v != "1" && v.to_lowercase() != "true")
            .unwrap_or(true);

        Self {
            relay_url,
            public_addresses,
            enable_dht,
            enable_mdns,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct GraphchanPaths {
    pub base: PathBuf,
    pub data_dir: PathBuf,
    pub db_path: PathBuf,
    pub files_dir: PathBuf,
    pub uploads_dir: PathBuf,
    pub downloads_dir: PathBuf,
    pub blobs_dir: PathBuf,
    pub keys_dir: PathBuf,
    pub gpg_dir: PathBuf,
    pub gpg_private_key: PathBuf,
    pub gpg_public_key: PathBuf,
    pub iroh_key_path: PathBuf,
    pub logs_dir: PathBuf,
}

impl GraphchanPaths {
    pub fn discover() -> Result<Self> {
        let exe_path = std::env::current_exe()
            .map_err(|err| anyhow!("failed to resolve current executable: {err}"))?;
        let base = exe_path
            .parent()
            .ok_or_else(|| anyhow!("executable path missing parent"))?
            .to_path_buf();
        Self::from_base_dir(base)
    }

    pub fn from_base_dir<P: AsRef<Path>>(base: P) -> Result<Self> {
        let base = base.as_ref().to_path_buf();
        let data_dir = base.join("data");
        let db_path = data_dir.join("graphchan.db");
        let files_dir = base.join("files");
        let uploads_dir = files_dir.join("uploads");
        let downloads_dir = files_dir.join("downloads");
        let blobs_dir = base.join("blobs");
        let keys_dir = base.join("keys");
        let gpg_dir = keys_dir.join("gpg");
        let gpg_private_key = gpg_dir.join("private.asc");
        let gpg_public_key = gpg_dir.join("public.asc");
        let iroh_key_path = keys_dir.join("iroh.key");
        let logs_dir = base.join("logs");

        Ok(Self {
            base,
            data_dir,
            db_path,
            files_dir,
            uploads_dir,
            downloads_dir,
            blobs_dir,
            keys_dir,
            gpg_dir,
            gpg_private_key,
            gpg_public_key,
            iroh_key_path,
            logs_dir,
        })
    }
}
