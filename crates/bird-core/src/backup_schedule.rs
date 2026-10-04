use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

const SECS_PER_MIN: u32 = 60;
const SECS_PER_HOUR: u32 = 60 * SECS_PER_MIN;
const SECS_PER_DAY: u32 = 24 * SECS_PER_HOUR;

// a service's volumes are copied this often on their own, plus how many of those copies stay
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct BackupSchedule {
    pub every: BackupInterval,
    pub keep: BackupKeep,
}

// every backup pauses the service's machines, so more often than hourly is refused
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(
    feature = "openapi",
    schema(value_type = u32, minimum = 3600, maximum = 2_592_000, description = "Seconds between backups")
)]
#[serde(try_from = "u32", into = "u32")]
pub struct BackupInterval(u32);

impl BackupInterval {
    pub const DAILY: Self = Self(SECS_PER_DAY);
    const RANGE: std::ops::RangeInclusive<u32> = SECS_PER_HOUR..=30 * SECS_PER_DAY;

    #[must_use]
    pub const fn secs(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl TryFrom<u32> for BackupInterval {
    type Error = ValidationError;

    fn try_from(secs: u32) -> Result<Self, Self::Error> {
        if Self::RANGE.contains(&secs) {
            Ok(Self(secs))
        } else {
            Err(ValidationError::BackupInterval(secs.to_string()))
        }
    }
}

impl FromStr for BackupInterval {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ValidationError::BackupInterval(s.to_owned());
        let lower = s.trim().to_ascii_lowercase();
        let (digits, scale) = if let Some(digits) = lower.strip_suffix('d') {
            (digits, SECS_PER_DAY)
        } else if let Some(digits) = lower.strip_suffix('h') {
            (digits, SECS_PER_HOUR)
        } else if let Some(digits) = lower.strip_suffix('m') {
            (digits, SECS_PER_MIN)
        } else {
            return Err(invalid());
        };
        let amount: u32 = digits.parse().map_err(|_| invalid())?;
        let secs = amount.checked_mul(scale).ok_or_else(invalid)?;
        Self::try_from(secs).map_err(|_| invalid())
    }
}

impl From<BackupInterval> for u32 {
    fn from(interval: BackupInterval) -> Self {
        interval.0
    }
}

impl fmt::Display for BackupInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_multiple_of(SECS_PER_DAY) {
            write!(f, "{}d", self.0 / SECS_PER_DAY)
        } else if self.0.is_multiple_of(SECS_PER_HOUR) {
            write!(f, "{}h", self.0 / SECS_PER_HOUR)
        } else {
            write!(f, "{}m", self.0 / SECS_PER_MIN)
        }
    }
}

// how many scheduled backups stay; older ones are deleted, manual and restore backups never are
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = u16, minimum = 1, maximum = 1000))]
#[serde(try_from = "u16", into = "u16")]
pub struct BackupKeep(u16);

impl BackupKeep {
    pub const WEEK: Self = Self(7);
    const RANGE: std::ops::RangeInclusive<u16> = 1..=1000;

    #[must_use]
    pub fn count(self) -> usize {
        usize::from(self.0)
    }
}

impl TryFrom<u16> for BackupKeep {
    type Error = ValidationError;

    fn try_from(count: u16) -> Result<Self, Self::Error> {
        if Self::RANGE.contains(&count) {
            Ok(Self(count))
        } else {
            Err(ValidationError::BackupKeep(count.to_string()))
        }
    }
}

impl FromStr for BackupKeep {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let count: u16 = s
            .trim()
            .parse()
            .map_err(|_| ValidationError::BackupKeep(s.to_owned()))?;
        Self::try_from(count)
    }
}

impl From<BackupKeep> for u16 {
    fn from(keep: BackupKeep) -> Self {
        keep.0
    }
}

impl fmt::Display for BackupKeep {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_intervals() {
        let secs = |text: &str| text.parse::<BackupInterval>().map(BackupInterval::secs);
        assert_eq!(secs("1d").unwrap(), 86_400);
        assert_eq!(secs("6H").unwrap(), 21_600);
        assert_eq!(secs("90m").unwrap(), 5_400);
        assert_eq!("24h".parse::<BackupInterval>().unwrap().to_string(), "1d");
        assert_eq!("12h".parse::<BackupInterval>().unwrap().to_string(), "12h");
        assert_eq!("90m".parse::<BackupInterval>().unwrap().to_string(), "90m");
        for bad in ["", "30m", "59m", "31d", "1w", "1", "-1d", "soon"] {
            assert!(bad.parse::<BackupInterval>().is_err(), "{bad}");
        }
    }

    #[test]
    fn keeps_one_to_a_thousand() {
        assert_eq!("7".parse::<BackupKeep>().unwrap(), BackupKeep::WEEK);
        assert_eq!("1000".parse::<BackupKeep>().unwrap().count(), 1000);
        for bad in ["0", "1001", "-1", "all", ""] {
            assert!(bad.parse::<BackupKeep>().is_err(), "{bad}");
        }
    }
}
