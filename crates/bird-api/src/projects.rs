use bird_core::Name;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ProjectSummary {
    pub name: Name,
    pub org: Name,
    pub environments: Vec<Name>,
}

/// A new project, which starts with a `production` environment
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateProject {
    pub name: Name,
    /// Org that owns it; left out, the one org you administer, or `default` for server admins
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub org: Option<Name>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateEnvironment {
    pub name: Name,
}
