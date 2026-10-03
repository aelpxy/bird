use bird_core::{DeploymentId, DeploymentStatus, ImageRef, Port};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentInfo {
    pub id: DeploymentId,
    pub image: ImageRef,
    pub port: Port,
    pub status: DeploymentStatus,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct RollbackRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_id: Option<DeploymentId>,
}
