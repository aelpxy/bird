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
    #[error("api token file {0} is invalid, delete it to generate a new token")]
    InvalidToken(PathBuf),
    #[error("the database worker has stopped")]
    DbClosed,
    #[error("service {0} not found")]
    ServiceNotFound(Name),
    #[error("service {0} has no running machines")]
    NoMachines(Name),
    #[error("another operation on {0} is in progress, try again shortly")]
    Busy(Name),
    #[error("{reason}")]
    Unhealthy { reason: String, logs: Vec<String> },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
