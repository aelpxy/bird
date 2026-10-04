use std::net::SocketAddr;
use std::path::PathBuf;

use bird_core::{EnvKey, Name};

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
    #[error(transparent)]
    Reference(#[from] bird_core::reference::ReferenceError),
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
    #[error("deployment {0} not found")]
    DeploymentNotFound(bird_core::DeploymentId),
    #[error("deployment {id} is {status}, only active or superseded deployments can be restored")]
    NotRollbackable {
        id: bird_core::DeploymentId,
        status: bird_core::DeploymentStatus,
    },
    #[error("service {0} has no earlier deployment to roll back to")]
    NoRollbackTarget(Name),
    #[error("{0} has a volume, which only one machine may use at a time")]
    VolumeNeedsSingleMachine(Name),
    #[error("volume {volume} is mounted at {was}, it cannot move to {now}")]
    VolumePathChanged {
        volume: Name,
        was: bird_core::MountPath,
        now: bird_core::MountPath,
    },
    #[error(
        "volume {volume} holds data written by {was}, running {now} on it could corrupt it; migrate the data first, then pass --allow-image-change"
    )]
    ImageChange {
        volume: Name,
        was: String,
        now: String,
    },
    #[error("{} is referenced by the variables of {}, remove those references first", .0, .1.iter().map(Name::as_str).collect::<Vec<_>>().join(", "))]
    HasDependents(Name, Vec<Name>),
    #[error("{0} has volumes, pass --purge to delete them together with their data")]
    HasVolumes(Name),
    #[error("volume {0} no longer exists in podman, its data was removed outside bird")]
    VolumeMissing(Name),
    #[error("no template named {0}, run `bird templates` to list them")]
    TemplateNotFound(Name),
    #[error("built-in template is invalid: {0}")]
    Template(String),
    #[error("a service named {0} already exists, pick another with --name")]
    ServiceExists(Name),
    #[error("domain {0} is already routed to a service")]
    DomainTaken(bird_core::Hostname),
    #[error("{0} has no variable {1}")]
    VariableNotFound(Name, EnvKey),
    #[error("image {0} is no longer on this server, deploy from source again to rebuild it")]
    LocalImageMissing(bird_core::ImageRef),
    #[error("build context is larger than {} MiB, exclude more with .dockerignore", .0 / 1024 / 1024)]
    ContextTooLarge(usize),
    #[error("invalid build args: {0}")]
    InvalidBuildArgs(String),
    #[error("build failed: {0}")]
    BuildFailed(String),
    #[error("registry username and password must not be empty")]
    EmptyCredentials,
    #[error("domain {0} is not attached to this service")]
    DomainNotFound(bird_core::Hostname),
    #[error("{0} is stopped, start it with `bird start`")]
    ServiceStopped(Name),
    #[error("{0} has no running machines, see `bird status`")]
    NoMachines(Name),
    #[error("{service} has no running machine matching {machine:?}, see `bird status`")]
    MachineNotFound { service: Name, machine: String },
    #[error("{0:?} matches more than one machine, give more of its id")]
    AmbiguousMachine(String),
    #[error("{0} has never been deployed, so there is no image to run")]
    NeverDeployed(Name),
    #[error("command failed: {0}")]
    CommandFailed(String),
    #[error("too many terminals are open, close one and try again")]
    TooManyTerminals,
    #[error("a terminal needs a connection upgraded to bird-tty")]
    UpgradeRequired,
    #[error("invalid terminal request: {0}")]
    InvalidTtyRequest(String),
    #[error("{0} has no volumes, there is nothing to back up")]
    NothingToBackUp(Name),
    #[error("backup {0} not found")]
    BackupNotFound(bird_core::BackupId),
    #[error("backup is kept in {0} storage, which this birdd is not set up to use")]
    BackupStorageMismatch(String),
    #[error("backup data {0} is missing from storage")]
    BackupDataMissing(String),
    #[error(
        "the backup has volume {volume}, which {service} does not have; deploy it with -v {volume}:<path> first"
    )]
    BackupVolumeMissing { service: Name, volume: Name },
    #[error("backup failed: {0}")]
    BackupFailed(String),
    #[error("restore failed: {reason}; the data from before the restore is in backup {safety}")]
    RestoreFailed {
        safety: bird_core::BackupId,
        reason: String,
    },
    #[error("another operation on {0} is in progress, try again shortly")]
    Busy(Name),
    #[error("{reason}")]
    Unhealthy { reason: String, logs: Vec<String> },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
