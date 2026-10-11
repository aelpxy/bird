use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

const SECS_PER_MIN: u32 = 60;

/// How a service's machines are told to stop
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum KillSignal {
    #[default]
    #[serde(rename = "SIGTERM")]
    Term,
    #[serde(rename = "SIGINT")]
    Int,
    #[serde(rename = "SIGQUIT")]
    Quit,
    #[serde(rename = "SIGHUP")]
    Hup,
    #[serde(rename = "SIGUSR1")]
    Usr1,
    #[serde(rename = "SIGUSR2")]
    Usr2,
}

impl KillSignal {
    const ALL: [Self; 6] = [
        Self::Term,
        Self::Int,
        Self::Quit,
        Self::Hup,
        Self::Usr1,
        Self::Usr2,
    ];

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Term => "SIGTERM",
            Self::Int => "SIGINT",
            Self::Quit => "SIGQUIT",
            Self::Hup => "SIGHUP",
            Self::Usr1 => "SIGUSR1",
            Self::Usr2 => "SIGUSR2",
        }
    }

    /// The signal's number on Linux, which podman takes
    #[must_use]
    pub const fn number(self) -> u8 {
        match self {
            Self::Hup => 1,
            Self::Int => 2,
            Self::Quit => 3,
            Self::Usr1 => 10,
            Self::Usr2 => 12,
            Self::Term => 15,
        }
    }
}

impl FromStr for KillSignal {
    type Err = ValidationError;

    // SIGINT, sigint and INT all name the same signal
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let upper = s.trim().to_ascii_uppercase();
        let name = upper.strip_prefix("SIG").unwrap_or(&upper);
        Self::ALL
            .into_iter()
            .find(|signal| signal.as_str().strip_prefix("SIG") == Some(name))
            .ok_or_else(|| {
                ValidationError::Stop(format!(
                    "unknown signal {s:?}, use SIGTERM, SIGINT, SIGQUIT, SIGHUP, SIGUSR1 or SIGUSR2"
                ))
            })
    }
}

impl fmt::Display for KillSignal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

macro_rules! seconds {
    ($(#[$doc:meta])* $name:ident, $min:literal, $max:literal, $default:literal, $what:literal) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[cfg_attr(feature = "openapi", schema(value_type = u32))]
        #[serde(try_from = "u32", into = "u32")]
        pub struct $name(u32);

        impl $name {
            pub const DEFAULT: Self = Self($default);
            pub const MAX: Self = Self($max);

            #[must_use]
            pub const fn secs(self) -> u32 {
                self.0
            }

            #[must_use]
            pub fn duration(self) -> Duration {
                Duration::from_secs(u64::from(self.0))
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::DEFAULT
            }
        }

        impl TryFrom<u32> for $name {
            type Error = ValidationError;

            fn try_from(secs: u32) -> Result<Self, Self::Error> {
                if ($min..=$max).contains(&secs) {
                    Ok(Self(secs))
                } else {
                    Err(ValidationError::Stop(format!(
                        "{} must be {}s to {}m, not {secs}s",
                        $what,
                        $min,
                        $max / SECS_PER_MIN
                    )))
                }
            }
        }

        impl From<$name> for u32 {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl FromStr for $name {
            type Err = ValidationError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                let secs = parse_secs(s).ok_or_else(|| {
                    ValidationError::Stop(format!("{} {s:?}: use seconds or minutes, like 30s or 2m", $what))
                })?;
                Self::try_from(secs)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                if self.0 != 0 && self.0.is_multiple_of(SECS_PER_MIN) {
                    write!(f, "{}m", self.0 / SECS_PER_MIN)
                } else {
                    write!(f, "{}s", self.0)
                }
            }
        }
    };
}

seconds!(
    /// How long old machines keep running after traffic moves off them, so requests finish
    DrainTime, 0, 300, 5, "drain"
);
seconds!(
    /// How long a machine has to exit after its kill signal before it is killed
    KillTimeout, 1, 300, 10, "kill_timeout"
);

fn parse_secs(s: &str) -> Option<u32> {
    let lower = s.trim().to_ascii_lowercase();
    let (digits, scale) = match lower.strip_suffix('m') {
        Some(digits) => (digits, SECS_PER_MIN),
        None => (lower.strip_suffix('s').unwrap_or(&lower), 1),
    };
    digits.trim().parse::<u32>().ok()?.checked_mul(scale)
}

/// How a service's old machines are stopped; the default is a short drain and SIGTERM
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(default, deny_unknown_fields)]
pub struct StopSettings {
    /// Seconds old machines keep serving requests in flight after traffic moves, 0 to 300
    pub drain: DrainTime,
    pub kill_signal: KillSignal,
    /// Seconds to exit after the signal before being killed, 1 to 300
    pub kill_timeout: KillTimeout,
}

impl StopSettings {
    #[must_use]
    pub fn is_default(&self) -> bool {
        *self == Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_signals_however_they_are_written() {
        for text in ["SIGINT", "sigint", "INT", " int "] {
            assert_eq!(text.parse::<KillSignal>().unwrap(), KillSignal::Int);
        }
        assert!("SIGKILL".parse::<KillSignal>().is_err());
        assert_eq!(
            serde_json::to_string(&KillSignal::Usr1).unwrap(),
            "\"SIGUSR1\""
        );
        assert_eq!(KillSignal::Term.number(), 15);
    }

    #[test]
    fn reads_durations_in_seconds_or_minutes_within_bounds() {
        assert_eq!("2m".parse::<KillTimeout>().unwrap().secs(), 120);
        assert_eq!("45".parse::<KillTimeout>().unwrap().secs(), 45);
        assert_eq!("0s".parse::<DrainTime>().unwrap().secs(), 0);
        assert!("0s".parse::<KillTimeout>().is_err());
        assert!("6m".parse::<DrainTime>().is_err());
        assert!("soon".parse::<DrainTime>().is_err());
        assert_eq!(KillTimeout::try_from(120).unwrap().to_string(), "2m");
    }

    #[test]
    fn defaults_fill_what_is_left_out() {
        let settings: StopSettings = serde_json::from_str(r#"{"kill_timeout":30}"#).unwrap();
        assert_eq!(settings.kill_timeout.secs(), 30);
        assert_eq!(
            (settings.drain, settings.kill_signal),
            (DrainTime::DEFAULT, KillSignal::Term)
        );
        assert!(StopSettings::default().is_default());
    }
}
