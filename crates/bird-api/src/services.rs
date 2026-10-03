use bird_core::{
    DeploymentId, DeploymentStatus, Hostname, ImageRef, MachineId, MachineState, Name, Port,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServiceSummary {
    pub name: Name,
    pub image: ImageRef,
    pub port: Port,
    pub domains: Vec<Hostname>,
    pub deployment: Option<DeploymentSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentSummary {
    pub id: DeploymentId,
    pub status: DeploymentStatus,
    pub machines: Vec<MachineSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineSummary {
    pub id: MachineId,
    pub state: MachineState,
}
