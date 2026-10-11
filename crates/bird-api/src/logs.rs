use bird_core::MachineId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct LogEntry {
    pub machine: MachineId,
    pub stream: LogStream,
    pub text: String,
    /// When podman recorded the line, in unix milliseconds
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<i64>,
}
