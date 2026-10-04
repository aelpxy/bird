use std::fmt;

use bird_core::{Name, SessionId};
use serde::{Deserialize, Serialize};

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct LoginRequest {
    pub username: Name,
    pub password: String,
    /// A code from the authenticator app or a recovery code, for users with two-factor on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
}

impl fmt::Debug for LoginRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginRequest")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum LoginResponse {
    /// The session token to send as a bearer token, shown only this once
    SignedIn {
        token: String,
        user: Name,
        expires_at: i64,
    },
    /// The password was right; send it again with a code
    TwoFactorRequired,
}

impl fmt::Debug for LoginResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::SignedIn {
                user, expires_at, ..
            } => f
                .debug_struct("SignedIn")
                .field("user", user)
                .field("expires_at", expires_at)
                .finish_non_exhaustive(),
            Self::TwoFactorRequired => f.write_str("TwoFactorRequired"),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct SetPassword {
    /// Needed when changing your own password once you have one; admins resetting someone else's leave it out
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current: Option<String>,
    pub new: String,
}

impl fmt::Debug for SetPassword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SetPassword(<redacted>)")
    }
}

/// The secret to add to an authenticator app, by its uri as a qr code or by hand
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct TwoFactorSetup {
    pub secret: String,
    pub uri: String,
}

impl fmt::Debug for TwoFactorSetup {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("TwoFactorSetup(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct TwoFactorCode {
    pub code: String,
}

/// Single-use codes for signing in without the authenticator, shown only this once
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RecoveryCodes {
    pub codes: Vec<String>,
}

impl fmt::Debug for RecoveryCodes {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("RecoveryCodes(<redacted>)")
    }
}

#[derive(Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct DisableTwoFactor {
    /// Needed when turning off your own; admins resetting someone else's leave it out
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub password: Option<String>,
}

impl fmt::Debug for DisableTwoFactor {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("DisableTwoFactor(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct SessionSummary {
    pub id: SessionId,
    pub created_at: i64,
    pub last_used_at: i64,
    pub expires_at: i64,
    pub address: Option<String>,
    pub agent: Option<String>,
    /// The session this request came in with
    pub current: bool,
}
