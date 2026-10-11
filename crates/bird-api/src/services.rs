use bird_core::{
    BackupSchedule, Command, CpuLimit, DeploymentId, DeploymentStatus, HealthCheck, HealthTimeout,
    Hostname, ImageRef, MachineId, MachineState, MemoryLimit, MetricPoint, MetricsRange, Name,
    Port, Replicas, ServiceState, StopSettings,
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
    /// Command the machines run instead of the image's own; left out, the image's
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<Command>,
    /// Runs once per deploy before new machines start
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release: Option<Command>,
    pub domains: Vec<Hostname>,
    pub volumes: Vec<VolumeSpec>,
    pub backup_schedule: Option<BackupSchedule>,
    pub deployment: Option<DeploymentSummary>,
    /// The newest deployment when it failed; an older active one may still be serving
    #[serde(default)]
    pub failed_deploy: Option<FailedDeploy>,
    /// Services in the same environment whose variables this one reads
    #[serde(default)]
    pub references: Vec<Name>,
    /// How old machines are drained and stopped
    #[serde(default)]
    pub stop: StopSettings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct FailedDeploy {
    pub id: DeploymentId,
    pub created_at: i64,
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

/// A service's usage over a range, from samples birdd takes every 30 seconds
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ServiceMetrics {
    pub range: MetricsRange,
    /// Seconds each point covers
    pub step_secs: i64,
    /// Oldest first; steps with no running machine are left out
    pub points: Vec<MetricPoint>,
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
