use std::net::SocketAddr;
use std::path::PathBuf;

use bird_core::Name;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Store(#[from] bird_store::Error),
    #[error(transparent)]
    Podman(#[from] bird_podman::Error),
    #[error(transparent)]
    Validation(#[from] bird_core::ValidationError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(
        "cannot listen on {0}: ports below 1024 need `sysctl net.ipv4.ip_unprivileged_port_start=80`, or pick a port like 8080"
    )]
    PrivilegedPort(SocketAddr),
    #[error("cannot listen on {addr}: {source}")]
    Bind {
        addr: SocketAddr,
        source: std::io::Error,
    },
    #[error("acme: {0}")]
    Acme(#[from] instant_acme::Error),
    #[error("invalid acme account data: {0}")]
    AcmeAccount(#[from] serde_json::Error),
    #[error("invalid certificate: {0}")]
    Certificate(String),
    #[error("api token file {0} is invalid, delete it to generate a new token")]
    InvalidToken(PathBuf),
    #[error("the database worker has stopped")]
    DbClosed,
    #[error("service {0} not found")]
    ServiceNotFound(Name),
    #[error("domain {0} is already routed to a service")]
    DomainTaken(bird_core::Hostname),
    #[error("domain {0} is not attached to this service")]
    DomainNotFound(bird_core::Hostname),
    #[error("service {0} has no running machines")]
    NoMachines(Name),
    #[error("another operation on {0} is in progress, try again shortly")]
    Busy(Name),
    #[error("{reason}")]
    Unhealthy { reason: String, logs: Vec<String> },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
