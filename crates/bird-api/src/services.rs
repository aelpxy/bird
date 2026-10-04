use bird_core::{
    BackupSchedule, CpuLimit, DeploymentId, DeploymentStatus, HealthCheck, HealthTimeout, Hostname,
    ImageRef, MachineId, MachineState, MemoryLimit, Name, Port, Replicas, ServiceState,
};
use serde::{Deserialize, Serialize};

use crate::VolumeSpec;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ServiceSummary {
    pub name: Name,
    /// Stopped services keep their machines stopped until started again
    pub state: ServiceState,
    pub image: ImageRef,
    pub port: Port,
    pub replicas: Replicas,
    pub memory: MemoryLimit,
    pub cpus: CpuLimit,
    pub health: HealthCheck,
    pub health_timeout: HealthTimeout,
    pub domains: Vec<Hostname>,
    pub volumes: Vec<VolumeSpec>,
    pub backup_schedule: Option<BackupSchedule>,
    pub deployment: Option<DeploymentSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeploymentSummary {
    pub id: DeploymentId,
    pub status: DeploymentStatus,
    pub created_at: i64,
    pub machines: Vec<MachineSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MachineSummary {
    pub id: MachineId,
    pub state: MachineState,
    /// When the machine entered its current state, in unix seconds
    pub updated_at: i64,
    /// Only sampled when asked for, and only for running machines
    #[serde(default)]
    pub stats: Option<MachineStats>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MachineStats {
    /// Thousandths of a core in use, measured over half a second
    pub cpu_millicores: u64,
    pub memory_bytes: u64,
    /// Received since the machine started
    pub net_rx_bytes: u64,
    /// Sent since the machine started
    pub net_tx_bytes: u64,
    pub processes: u64,
}
