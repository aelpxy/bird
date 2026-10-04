use std::net::SocketAddr;
use std::path::PathBuf;

use bird_core::{BackupInterval, BackupKeep};
use clap::Parser;

#[derive(Debug, Clone, Parser)]
#[command(
    name = "birdd",
    version,
    about = "bird daemon: API, proxy and machine supervisor"
)]
pub struct Config {
    #[arg(long, env = "BIRD_API_ADDR", default_value = "127.0.0.1:7070")]
    pub api_addr: SocketAddr,

    #[arg(long, env = "BIRD_PROXY_ADDR", default_value = "0.0.0.0:80")]
    pub proxy_addr: SocketAddr,

    #[arg(long, env = "BIRD_HTTPS_ADDR", default_value = "0.0.0.0:443")]
    pub https_addr: SocketAddr,

    /// ACME directory URL, enables automatic https when set
    #[arg(long, env = "BIRD_ACME_DIRECTORY")]
    pub acme_directory: Option<String>,

    /// Contact email for the ACME account
    #[arg(long, env = "BIRD_ACME_EMAIL")]
    pub acme_email: Option<String>,

    /// Extra root CA trusted for the ACME directory, for test CAs like Pebble
    #[arg(long, env = "BIRD_ACME_CA_CERT")]
    pub acme_ca_cert: Option<PathBuf>,

    #[arg(long, env = "BIRD_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

    /// Directory for volume backups, defaults to `backups` in the data directory
    #[arg(long, env = "BIRD_BACKUP_DIR")]
    pub backup_dir: Option<PathBuf>,

    /// Keep backups in this S3-compatible bucket instead of a directory
    #[arg(
        long,
        env = "BIRD_S3_BUCKET",
        conflicts_with = "backup_dir",
        requires_all = ["s3_access_key_id", "s3_secret_access_key"]
    )]
    pub s3_bucket: Option<String>,

    /// S3 endpoint for services other than AWS, like https://<account>.r2.cloudflarestorage.com
    #[arg(long, env = "BIRD_S3_ENDPOINT", requires = "s3_bucket")]
    pub s3_endpoint: Option<String>,

    #[arg(long, env = "BIRD_S3_REGION", default_value = "us-east-1")]
    pub s3_region: String,

    /// Folder inside the bucket to keep backups under
    #[arg(long, env = "BIRD_S3_PREFIX", requires = "s3_bucket")]
    pub s3_prefix: Option<String>,

    #[arg(long, env = "BIRD_S3_ACCESS_KEY_ID", requires = "s3_bucket")]
    pub s3_access_key_id: Option<String>,

    /// Set it through the environment, other users on the server can read command lines
    #[arg(
        long,
        env = "BIRD_S3_SECRET_ACCESS_KEY",
        hide_env_values = true,
        requires = "s3_bucket"
    )]
    pub s3_secret_access_key: Option<Secret>,

    /// How often to copy bird.db into backup storage, like 6h or 1d
    #[arg(long, env = "BIRD_DB_BACKUP_EVERY", default_value = "1d")]
    pub db_backup_every: BackupInterval,

    /// How many copies of bird.db to keep
    #[arg(long, env = "BIRD_DB_BACKUP_KEEP", default_value = "7")]
    pub db_backup_keep: BackupKeep,

    /// Do not copy bird.db into backup storage
    #[arg(long, env = "BIRD_NO_DB_BACKUP")]
    pub no_db_backup: bool,

    #[arg(long, env = "BIRD_PODMAN_SOCKET")]
    pub podman_socket: Option<PathBuf>,

    #[arg(long, env = "BIRD_NETWORK", default_value = "bird")]
    pub network: String,
}

impl Config {
    #[must_use]
    pub fn data_dir(&self) -> PathBuf {
        self.data_dir.clone().unwrap_or_else(default_data_dir)
    }
}

// kept out of debug output, so printing the config cannot leak it
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl std::str::FromStr for Secret {
    type Err = std::convert::Infallible;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Ok(Self(value.to_owned()))
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(..)")
    }
}

fn default_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map_or_else(|| PathBuf::from("/var/lib/bird"), |base| base.join("bird"))
}
