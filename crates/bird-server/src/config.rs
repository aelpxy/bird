use std::net::SocketAddr;
use std::path::PathBuf;

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

    #[arg(long, env = "BIRD_PROXY_ADDR", default_value = "0.0.0.0:8080")]
    pub proxy_addr: SocketAddr,

    #[arg(long, env = "BIRD_DATA_DIR")]
    pub data_dir: Option<PathBuf>,

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

fn default_data_dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/share")))
        .map_or_else(|| PathBuf::from("/var/lib/bird"), |base| base.join("bird"))
}
