use bird_core::{EnvKey, ImageRef, Name};
use serde::{Deserialize, Serialize};

use crate::DeployResponse;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TemplateSummary {
    pub name: Name,
    pub description: String,
    pub image: ImageRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreateFromTemplate {
    /// Service name, defaults to the template name
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<Name>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TemplateDeployResponse {
    pub deployment: DeployResponse,
    /// Variable other services reference to connect, like `DATABASE_URL`
    pub connection: Option<EnvKey>,
}
