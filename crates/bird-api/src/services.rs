use bird_core::{
    DeploymentId, DeploymentStatus, HealthCheck, Hostname, ImageRef, MachineId, MachineState, Name,
    Port, Replicas,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ServiceSummary {
    pub name: Name,
    pub image: ImageRef,
    pub port: Port,
    pub replicas: Replicas,
    pub health: HealthCheck,
    pub domains: Vec<Hostname>,
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
