use bird_core::{DeploymentId, DeploymentStatus, ImageRef, Port};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeploymentInfo {
    pub id: DeploymentId,
    pub image: ImageRef,
    pub port: Port,
    pub status: DeploymentStatus,
    pub variables: usize,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RollbackRequest {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deployment_id: Option<DeploymentId>,
}
