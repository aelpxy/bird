use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

const SECS_PER_MIN: u32 = 60;
const MAX_PATH_BYTES: usize = 255;

// how birdd decides a machine is ready and still alive
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum HealthCheck {
    Http,
    Path(HealthPath),
    Tcp,
}

#[cfg(feature = "openapi")]
impl utoipa::PartialSchema for HealthCheck {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(
                "http (any HTTP response on /), tcp (accepts connections) or a path like /healthz that must answer 2xx",
            ))
            .examples(["/healthz"])
            .into()
    }
}

#[cfg(feature = "openapi")]
impl utoipa::ToSchema for HealthCheck {}

impl HealthCheck {
    #[must_use]
    pub const fn path(&self) -> Option<&HealthPath> {
        match self {
            Self::Path(path) => Some(path),
            Self::Http | Self::Tcp => None,
        }
    }
}

impl FromStr for HealthCheck {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "http" => Ok(Self::Http),
            "tcp" => Ok(Self::Tcp),
            path if path.starts_with('/') => path.parse().map(Self::Path),
            other => Err(ValidationError::Health(other.to_owned())),
        }
    }
}

impl TryFrom<String> for HealthCheck {
    type Error = ValidationError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl From<HealthCheck> for String {
    fn from(check: HealthCheck) -> Self {
        check.to_string()
    }
}

impl fmt::Display for HealthCheck {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Http => f.write_str("http"),
            Self::Path(path) => f.write_str(path.as_str()),
            Self::Tcp => f.write_str("tcp"),
        }
    }
}

// written into the probe's request line, so only visible ascii without spaces or fragments
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct HealthPath(String);

impl HealthPath {
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for HealthPath {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let ok = s.starts_with('/')
            && s.len() <= MAX_PATH_BYTES
            && s.bytes().all(|c| c.is_ascii_graphic() && c != b'#');
        if ok {
            Ok(Self(s.to_owned()))
        } else {
            Err(ValidationError::HealthPath(s.to_owned()))
        }
    }
}

impl fmt::Display for HealthPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// how long a new machine has to pass its first health check
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = u32, minimum = 1, maximum = 3600))]
#[serde(try_from = "u32", into = "u32")]
pub struct HealthTimeout(u32);

impl HealthTimeout {
    pub const DEFAULT: Self = Self(60);
    const RANGE: std::ops::RangeInclusive<u32> = 1..=3600;

    #[must_use]
    pub const fn secs(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl Default for HealthTimeout {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u32> for HealthTimeout {
    type Error = ValidationError;

    fn try_from(secs: u32) -> Result<Self, Self::Error> {
        if Self::RANGE.contains(&secs) {
            Ok(Self(secs))
        } else {
            Err(ValidationError::HealthTimeout(secs.to_string()))
        }
    }
}

impl FromStr for HealthTimeout {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ValidationError::HealthTimeout(s.to_owned());
        let lower = s.trim().to_ascii_lowercase();
        let (digits, scale) = match lower.strip_suffix('m') {
            Some(digits) => (digits, SECS_PER_MIN),
            None => (lower.strip_suffix('s').unwrap_or(&lower), 1),
        };
        let amount: u32 = digits.parse().map_err(|_| invalid())?;
        let secs = amount.checked_mul(scale).ok_or_else(invalid)?;
        Self::try_from(secs).map_err(|_| invalid())
    }
}

impl From<HealthTimeout> for u32 {
    fn from(timeout: HealthTimeout) -> Self {
        timeout.0
    }
}

impl fmt::Display for HealthTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_multiple_of(SECS_PER_MIN) {
            write!(f, "{}m", self.0 / SECS_PER_MIN)
        } else {
            write!(f, "{}s", self.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_checks() {
        assert_eq!("http".parse::<HealthCheck>().unwrap(), HealthCheck::Http);
        assert_eq!("tcp".parse::<HealthCheck>().unwrap(), HealthCheck::Tcp);
        let check: HealthCheck = "/healthz?full=1".parse().unwrap();
        assert_eq!(check.path().unwrap().as_str(), "/healthz?full=1");
        assert_eq!(check.to_string(), "/healthz?full=1");
        let bad_path = format!("/{}", "a".repeat(MAX_PATH_BYTES));
        for bad in [
            "",
            "grpc",
            "healthz",
            "/a b",
            "/a\r\nX: y",
            "/a#b",
            &bad_path,
        ] {
            assert!(bad.parse::<HealthCheck>().is_err(), "{bad:?}");
        }
    }

    #[test]
    fn parses_and_prints_timeouts() {
        assert_eq!("90".parse::<HealthTimeout>().unwrap().secs(), 90);
        assert_eq!("90s".parse::<HealthTimeout>().unwrap().secs(), 90);
        assert_eq!("5M".parse::<HealthTimeout>().unwrap().secs(), 300);
        assert_eq!("120".parse::<HealthTimeout>().unwrap().to_string(), "2m");
        assert_eq!("45".parse::<HealthTimeout>().unwrap().to_string(), "45s");
        assert_eq!(HealthTimeout::DEFAULT.duration(), Duration::from_mins(1));
        for bad in ["", "0", "61m", "3601", "1h", "-5", "soon"] {
            assert!(bad.parse::<HealthTimeout>().is_err(), "{bad}");
        }
    }
}
