use std::fmt;
use std::net::SocketAddr;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::{
    Command, CpuLimit, DeploymentId, EnvKey, EnvironmentId, Hostname, ImageRef, MachineId,
    MemoryLimit, MountPath, Name, Port, ProjectId, Replicas, ServiceId, VolumeId,
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

string_enum!(HealthCheck, "health check" {
    Http => "http",
    Tcp => "tcp",
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Project {
    pub id: ProjectId,
    pub name: Name,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Environment {
    pub id: EnvironmentId,
    pub project_id: ProjectId,
    pub name: Name,
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
    fn variable_debug_hides_value() {
        let v = Variable {
            service_id: ServiceId::generate(),
            key: "SECRET".parse().unwrap(),
            value: "hunter2".to_owned(),
        };
        assert!(!format!("{v:?}").contains("hunter2"));
    }
}
