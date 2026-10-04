use bird_core::{BackupId, BackupTrigger, Name};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct BackupInfo {
    pub id: BackupId,
    pub service: Name,
    /// manual, or restore for the copy taken automatically before a restore
    pub trigger: BackupTrigger,
    /// Where the archives are kept, such as local
    pub storage: String,
    pub volumes: Vec<BackupVolumeInfo>,
    /// Unix seconds
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct BackupVolumeInfo {
    pub name: Name,
    pub size_bytes: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RestoreRequest {
    /// Restore data written by a different image than the service runs now
    #[serde(default)]
    pub allow_image_change: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RestoreResponse {
    pub restored: BackupId,
    /// Backup of the data that was replaced, to undo the restore
    pub safety_backup: BackupId,
}
