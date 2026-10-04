use bird_core::{Command, CronRunId, CronRunStatus, CronSchedule, CronTimeout, CronTrigger, Name};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CronJobSummary {
    pub name: Name,
    pub schedule: CronSchedule,
    pub command: Command,
    pub timeout: CronTimeout,
    /// Unix seconds of the next scheduled run, in UTC
    pub next_run_at: Option<i64>,
    pub last_run: Option<CronRunSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CronRunSummary {
    pub id: CronRunId,
    pub trigger: CronTrigger,
    pub status: CronRunStatus,
    pub exit_code: Option<i32>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
}

/// A run with the end of what it printed, or why it did not run
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CronRunDetail {
    #[serde(flatten)]
    pub run: CronRunSummary,
    pub output: String,
}
