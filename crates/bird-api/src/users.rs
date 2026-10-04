use std::fmt;
use std::num::NonZeroU16;

use bird_core::{Name, UserRole};
use serde::{Deserialize, Serialize};

/// Who the request's token belongs to; the token from birdd's data dir is `root`
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct Whoami {
    pub name: String,
    pub role: UserRole,
    #[serde(default)]
    pub has_password: bool,
    #[serde(default)]
    pub two_factor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct UserSummary {
    pub name: Name,
    pub role: UserRole,
    pub has_password: bool,
    pub two_factor: bool,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateUser {
    pub name: Name,
    #[serde(default = "member")]
    pub role: UserRole,
}

const fn member() -> UserRole {
    UserRole::Member
}

/// The new user and a first token for them, shown only this once
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct CreatedUser {
    pub user: UserSummary,
    pub token: IssuedToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CreateToken {
    pub name: Name,
    /// Days until the token stops working; left out, it works until deleted
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "openapi", schema(value_type = Option<u16>))]
    pub expires_in_days: Option<NonZeroU16>,
}

/// A token as it is listed; the secret itself is never shown again
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TokenSummary {
    pub name: Name,
    pub prefix: String,
    pub created_at: i64,
    pub last_used_at: Option<i64>,
    pub expires_at: Option<i64>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct IssuedToken {
    pub name: Name,
    /// The secret to send as a bearer token
    pub token: String,
    pub expires_at: Option<i64>,
}

impl fmt::Debug for IssuedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IssuedToken")
            .field("name", &self.name)
            .field("token", &"<redacted>")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}
