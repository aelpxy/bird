use std::collections::BTreeMap;

use bird_core::{
    BackupSchedule, Command, CpuLimit, DeploymentId, EnvKey, HealthCheck, HealthTimeout, Hostname,
    ImageRef, MemoryLimit, Name, Port, Replicas,
};
use serde::{Deserialize, Serialize};

use crate::VolumeSpec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeployRequest {
    pub name: Name,
    pub image: ImageRef,
    pub port: Port,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub domains: Vec<Hostname>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<EnvKey, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health: Option<HealthCheck>,
    /// Seconds a new machine has to pass its health check; left out, a new service gets 60 and an existing one keeps its timeout
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub health_timeout: Option<HealthTimeout>,
    /// Command to run instead of the image's; left out, an existing service keeps its saved one
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
    /// Forget the saved command and run the image's own again; not together with `command`
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub default_command: bool,
    /// Memory limit in MiB; left out, a new service gets 1024 and an existing one keeps its limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub memory: Option<MemoryLimit>,
    /// CPU limit in thousandths of a core; left out, a new service gets 1000 and an existing one keeps its limit
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cpus: Option<CpuLimit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub volumes: Vec<VolumeSpec>,
    /// Machines to run; left out, a new service gets 1 and an existing one keeps its count
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replicas: Option<Replicas>,
    /// Back the volumes up on a schedule; left out, an existing schedule is kept
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backup: Option<BackupSchedule>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_image_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeployResponse {
    pub service: Name,
    pub deployment_id: DeploymentId,
    pub image: ImageRef,
    pub domains: Vec<Hostname>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_fields() {
        let bad_name = r#"{"name":"Web","image":"nginx","port":80}"#;
        assert!(serde_json::from_str::<DeployRequest>(bad_name).is_err());
        let bad_port = r#"{"name":"web","image":"nginx","port":0}"#;
        assert!(serde_json::from_str::<DeployRequest>(bad_port).is_err());
        let bad_env = r#"{"name":"web","image":"nginx","port":80,"env":{"A-B":"1"}}"#;
        assert!(serde_json::from_str::<DeployRequest>(bad_env).is_err());
    }

    #[test]
    fn optional_fields_default() {
        let minimal = r#"{"name":"web","image":"nginx","port":80}"#;
        let request: DeployRequest = serde_json::from_str(minimal).unwrap();
        assert_eq!(request.domains, Vec::new());
        assert!(request.env.is_empty());
        assert_eq!(request.health, None);
    }

    #[test]
    fn health_checks_travel_as_strings() {
        let with_path =
            r#"{"name":"web","image":"nginx","port":80,"health":"/up","health_timeout":90}"#;
        let request: DeployRequest = serde_json::from_str(with_path).unwrap();
        assert_eq!(request.health.unwrap().to_string(), "/up");
        assert_eq!(request.health_timeout.unwrap().secs(), 90);
        let json = serde_json::to_string(&DeployRequest {
            health: Some(HealthCheck::Tcp),
            ..request
        })
        .unwrap();
        assert!(json.contains(r#""health":"tcp""#), "{json}");
        let bad = r#"{"name":"web","image":"nginx","port":80,"health":"up"}"#;
        assert!(serde_json::from_str::<DeployRequest>(bad).is_err());
    }
}
