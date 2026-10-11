use bird_core::Hostname;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct AddDomain {
    pub hostname: Hostname,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DomainStatus {
    pub hostname: Hostname,
    pub https: Https,
    /// Visitors are sent to this other domain of the service instead of the app
    pub redirect_to: Option<Hostname>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct UpdateDomain {
    /// Another domain of the same service to redirect to, or null to serve the app again
    pub redirect_to: Option<Hostname>,
}

/// Where the domain's certificate stands
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Https {
    /// Served over https; http redirects to it
    Issued { expires_at: i64 },
    /// birdd is asking for a certificate
    Pending,
    /// The last request failed and is retried at `retry_at`, both in unix seconds
    Failing { error: String, retry_at: i64 },
    /// birdd runs without an acme directory, so it serves plain http only
    Off,
    /// `*.localhost` names never get certificates
    Local,
}
