use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{
    BackupId, Command, CpuLimit, DeploymentId, EnvKey, EnvironmentId, HealthCheck, HealthTimeout,
    Hostname, ImageRef, MachineId, MemoryLimit, MountPath, Name, OrgId, Port, ProjectId,
    RegistryHost, Replicas, ServiceId, SessionId, TokenId, UserId, VolumeId,
};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown {kind} {value:?}")]
pub struct ParseEnumError {
    kind: &'static str,
    value: String,
}

macro_rules! string_enum {
    ($name:ident, $kind:literal { $($variant:ident => $s:literal),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[serde(rename_all = "lowercase")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            #[must_use]
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $s),+
                }
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl FromStr for $name {
            type Err = ParseEnumError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                match s {
                    $($s => Ok(Self::$variant),)+
                    _ => Err(ParseEnumError { kind: $kind, value: s.to_owned() }),
                }
            }
        }
    };
}

string_enum!(DeploymentStatus, "deployment status" {
    Pending => "pending",
    Deploying => "deploying",
    Active => "active",
    Superseded => "superseded",
    Failed => "failed",
});

// what the operator wants: a stopped service keeps its machines stopped and is left alone by the supervisor
string_enum!(ServiceState, "service state" {
    Running => "running",
    Stopped => "stopped",
});

string_enum!(MachineState, "machine state" {
    Created => "created",
    Starting => "starting",
    Running => "running",
    Stopping => "stopping",
    Stopped => "stopped",
    Failed => "failed",
    Destroyed => "destroyed",
});

// admins manage users; everyone manages their own tokens
string_enum!(UserRole, "user role" {
    Admin => "admin",
    Member => "member",
});

// what a user may do in an org: members work with services, admins also create and delete
// projects and environments, owners also manage who belongs
string_enum!(OrgRole, "org role" {
    Owner => "owner",
    Admin => "admin",
    Member => "member",
});

impl OrgRole {
    const fn rank(self) -> u8 {
        match self {
            Self::Member => 0,
            Self::Admin => 1,
            Self::Owner => 2,
        }
    }

    #[must_use]
    pub const fn at_least(self, needed: Self) -> bool {
        self.rank() >= needed.rank()
    }
}

string_enum!(BackupTrigger, "backup trigger" {
    Manual => "manual",
    Restore => "restore",
    Scheduled => "scheduled",
});

// password hashes and 2fa secrets are not part of it, so loading or logging a user never carries them
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub name: Name,
    pub role: UserRole,
    pub has_password: bool,
    pub two_factor: bool,
    pub created_at: i64,
}

// a signed-in browser or CLI; its secret is stored only as a hash, like an api token's
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub user_id: UserId,
    pub created_at: i64,
    pub last_used_at: i64,
    pub expires_at: i64,
    pub address: Option<String>,
    pub agent: Option<String>,
}

// the secret itself is never stored, only its sha-256, which is what requests are looked up by
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApiToken {
    pub id: TokenId,
    pub user_id: UserId,
    pub name: Name,
    // the start of the secret, so a user can tell their tokens apart
    pub prefix: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub expires_at: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Org {
    pub id: OrgId,
    pub name: Name,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub org_id: OrgId,
    pub name: Name,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: EnvironmentId,
    pub project_id: ProjectId,
    pub name: Name,
    // the podman network its machines share; `None` is the network birdd was started with
    pub network: Option<String>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Service {
    pub id: ServiceId,
    pub environment_id: EnvironmentId,
    pub name: Name,
    pub image: ImageRef,
    pub port: Port,
    pub replicas: Replicas,
    pub health: HealthCheck,
    pub health_timeout: HealthTimeout,
    pub state: ServiceState,
    pub command: Option<Command>,
    pub memory: MemoryLimit,
    pub cpus: CpuLimit,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Deployment {
    pub id: DeploymentId,
    pub service_id: ServiceId,
    pub image: ImageRef,
    pub port: Port,
    pub command: Option<Command>,
    pub status: DeploymentStatus,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Machine {
    pub id: MachineId,
    pub deployment_id: DeploymentId,
    pub container_id: Option<String>,
    pub address: Option<SocketAddr>,
    pub state: MachineState,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Domain {
    pub hostname: Hostname,
    pub service_id: ServiceId,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Volume {
    pub id: VolumeId,
    pub service_id: ServiceId,
    pub name: Name,
    pub mount_path: MountPath,
    pub lineage: Option<String>,
    pub created_at: i64,
}

impl Volume {
    #[must_use]
    pub fn podman_name(&self) -> String {
        format!("bird-volume-{}", self.id)
    }
}

// keyed by service name, not id, so backups outlive `bird rm --purge` and can restore a new service
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Backup {
    pub id: BackupId,
    pub environment_id: EnvironmentId,
    pub service: Name,
    pub trigger: BackupTrigger,
    pub storage: String,
    pub volumes: Vec<BackupVolume>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackupVolume {
    pub name: Name,
    pub lineage: Option<String>,
    pub key: String,
    pub size_bytes: u64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Registry {
    pub host: RegistryHost,
    pub username: String,
    pub password: String,
    pub insecure: bool,
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Registry")
            .field("host", &self.host)
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("insecure", &self.insecure)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Variable {
    pub service_id: ServiceId,
    pub key: EnvKey,
    pub value: String,
}

impl fmt::Debug for Variable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Variable")
            .field("service_id", &self.service_id)
            .field("key", &self.key)
            .field("value", &"<redacted>")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Certificate {
    pub hostname: Hostname,
    pub chain_pem: String,
    pub key_pem: String,
    pub not_after: i64,
}

impl fmt::Debug for Certificate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Certificate")
            .field("hostname", &self.hostname)
            .field("not_after", &self.not_after)
            .field("key_pem", &"<redacted>")
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enums_roundtrip() {
        for s in [
            MachineState::Created,
            MachineState::Running,
            MachineState::Destroyed,
        ] {
            assert_eq!(s.as_str().parse::<MachineState>().unwrap(), s);
        }
        assert!("bogus".parse::<DeploymentStatus>().is_err());
    }

    #[test]
    fn org_roles_include_the_ones_below() {
        assert!(OrgRole::Owner.at_least(OrgRole::Admin));
        assert!(OrgRole::Admin.at_least(OrgRole::Admin));
        assert!(!OrgRole::Member.at_least(OrgRole::Admin));
        assert!(OrgRole::Member.at_least(OrgRole::Member));
    }

    #[test]
    fn variable_debug_hides_value() {
        let v = Variable {
            service_id: ServiceId::generate(),
            key: "SECRET".parse().unwrap(),
            value: "hunter2".to_owned(),
        };
        assert!(!format!("{v:?}").contains("hunter2"));
    }
}
