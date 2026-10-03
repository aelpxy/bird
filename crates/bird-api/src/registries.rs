use bird_core::RegistryHost;
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RegistryLogin {
    pub username: String,
    pub password: String,
    /// Talk plain http and skip certificate checks, only for registries on a private network
    #[serde(default)]
    pub insecure: bool,
}

impl std::fmt::Debug for RegistryLogin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistryLogin")
            .field("username", &self.username)
            .field("password", &"<redacted>")
            .field("insecure", &self.insecure)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RegistrySummary {
    pub host: RegistryHost,
    pub username: String,
    pub insecure: bool,
}
