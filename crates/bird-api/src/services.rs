use bird_core::{
    CpuLimit, DeploymentId, DeploymentStatus, HealthCheck, HealthTimeout, Hostname, ImageRef,
    MachineId, MachineState, MemoryLimit, Name, Port, Replicas,
};
use serde::{Deserialize, Serialize};

use crate::VolumeSpec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ServiceSummary {
    pub name: Name,
    pub image: ImageRef,
    pub port: Port,
    pub replicas: Replicas,
    pub memory: MemoryLimit,
    pub cpus: CpuLimit,
    pub health: HealthCheck,
    pub health_timeout: HealthTimeout,
    pub domains: Vec<Hostname>,
    pub volumes: Vec<VolumeSpec>,
    pub deployment: Option<DeploymentSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeploymentSummary {
    pub id: DeploymentId,
    pub status: DeploymentStatus,
    pub machines: Vec<MachineSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MachineSummary {
    pub id: MachineId,
    pub state: MachineState,
}
