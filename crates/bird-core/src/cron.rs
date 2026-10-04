use std::fmt;
use std::str::FromStr;
use std::time::Duration;

use chrono::{DateTime, Utc};
use croner::Cron;
use croner::parser::{CronParser, Seconds, Year};
use serde::{Deserialize, Serialize};

use crate::{Command, CronJobId, CronRunId, Name, ServiceId, ValidationError};

const SECS_PER_MIN: u32 = 60;
const SECS_PER_HOUR: u32 = 60 * SECS_PER_MIN;

string_enum!(CronTrigger, "cron trigger" {
    Schedule => "schedule",
    Manual => "manual",
});

string_enum!(CronRunStatus, "cron run status" {
    Running => "running",
    Succeeded => "succeeded",
    Failed => "failed",
    TimedOut => "timed_out",
    // birdd stopped while it ran
    Interrupted => "interrupted",
    // not started: the previous run was still going, or the service was stopped
    Skipped => "skipped",
});

// a classic 5-field cron pattern, minute granularity, like "*/15 * * * *", or an alias like
// "@daily"; evaluated in utc
#[derive(Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CronSchedule {
    text: String,
    cron: Cron,
}

impl CronSchedule {
    // the first time it fires strictly after `unix_secs`
    #[must_use]
    pub fn next_after(&self, unix_secs: i64) -> Option<i64> {
        let after: DateTime<Utc> = DateTime::from_timestamp(unix_secs, 0)?;
        self.cron
            .find_next_occurrence(&after, false)
            .ok()
            .map(|next| next.timestamp())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }
}

#[cfg(feature = "openapi")]
impl utoipa::PartialSchema for CronSchedule {
    fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
        utoipa::openapi::ObjectBuilder::new()
            .schema_type(utoipa::openapi::schema::Type::String)
            .description(Some(
                "A 5-field cron pattern in UTC like */15 * * * *, or an alias like @daily",
            ))
            .examples(["0 3 * * *"])
            .into()
    }
}

#[cfg(feature = "openapi")]
impl utoipa::ToSchema for CronSchedule {}

impl FromStr for CronSchedule {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let text = s.trim().to_owned();
        let cron = CronParser::builder()
            .seconds(Seconds::Disallowed)
            .year(Year::Disallowed)
            .build()
            .parse(&text)
            .map_err(|err| ValidationError::CronSchedule(text.clone(), err.to_string()))?;
        Ok(Self { text, cron })
    }
}

impl TryFrom<String> for CronSchedule {
    type Error = ValidationError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        text.parse()
    }
}

impl From<CronSchedule> for String {
    fn from(schedule: CronSchedule) -> Self {
        schedule.text
    }
}

impl PartialEq for CronSchedule {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for CronSchedule {}

impl fmt::Display for CronSchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.text)
    }
}

impl fmt::Debug for CronSchedule {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CronSchedule({:?})", self.text)
    }
}

// how long one run may take before it is stopped
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = u32, minimum = 1, maximum = 86400))]
#[serde(try_from = "WrittenTimeout", into = "u32")]
pub struct CronTimeout(u32);

// seconds, or text like "30m" as bird.toml has it
#[derive(Deserialize)]
#[serde(untagged)]
enum WrittenTimeout {
    Secs(u32),
    Text(String),
}

impl TryFrom<WrittenTimeout> for CronTimeout {
    type Error = ValidationError;

    fn try_from(written: WrittenTimeout) -> Result<Self, Self::Error> {
        match written {
            WrittenTimeout::Secs(secs) => Self::try_from(secs),
            WrittenTimeout::Text(text) => text.parse(),
        }
    }
}

impl CronTimeout {
    pub const DEFAULT: Self = Self(SECS_PER_HOUR);
    const RANGE: std::ops::RangeInclusive<u32> = 1..=24 * SECS_PER_HOUR;

    #[must_use]
    pub const fn secs(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn duration(self) -> Duration {
        Duration::from_secs(u64::from(self.0))
    }
}

impl Default for CronTimeout {
    fn default() -> Self {
        Self::DEFAULT
    }
}

impl TryFrom<u32> for CronTimeout {
    type Error = ValidationError;

    fn try_from(secs: u32) -> Result<Self, Self::Error> {
        if Self::RANGE.contains(&secs) {
            Ok(Self(secs))
        } else {
            Err(ValidationError::CronTimeout(secs.to_string()))
        }
    }
}

impl FromStr for CronTimeout {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ValidationError::CronTimeout(s.to_owned());
        let lower = s.trim().to_ascii_lowercase();
        let (digits, scale) = if let Some(digits) = lower.strip_suffix('h') {
            (digits, SECS_PER_HOUR)
        } else if let Some(digits) = lower.strip_suffix('m') {
            (digits, SECS_PER_MIN)
        } else {
            (lower.strip_suffix('s').unwrap_or(&lower), 1)
        };
        let amount: u32 = digits.parse().map_err(|_| invalid())?;
        let secs = amount.checked_mul(scale).ok_or_else(invalid)?;
        Self::try_from(secs).map_err(|_| invalid())
    }
}

impl From<CronTimeout> for u32 {
    fn from(timeout: CronTimeout) -> Self {
        timeout.0
    }
}

impl fmt::Display for CronTimeout {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_multiple_of(SECS_PER_HOUR) {
            write!(f, "{}h", self.0 / SECS_PER_HOUR)
        } else if self.0.is_multiple_of(SECS_PER_MIN) {
            write!(f, "{}m", self.0 / SECS_PER_MIN)
        } else {
            write!(f, "{}s", self.0)
        }
    }
}

// a job as bird.toml and deploys describe it
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(deny_unknown_fields)]
pub struct CronJobSpec {
    pub name: Name,
    pub schedule: CronSchedule,
    pub command: Command,
    /// Seconds a run may take, or text like "30m"; left out, an hour
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<CronTimeout>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronJob {
    pub id: CronJobId,
    pub service_id: ServiceId,
    pub name: Name,
    pub schedule: CronSchedule,
    pub command: Command,
    pub timeout: CronTimeout,
    // the latest scheduled time already handled, so a time fires once
    pub last_due_at: Option<i64>,
    pub created_at: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CronRun {
    pub id: CronRunId,
    pub job_id: CronJobId,
    pub trigger: CronTrigger,
    pub status: CronRunStatus,
    pub exit_code: Option<i32>,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    // the end of what it printed, or why it did not run
    pub output: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-04 10:07:30 utc
    const NOW: i64 = 1_791_108_450;

    #[test]
    fn parses_five_fields_and_aliases_only() {
        for good in [
            "*/15 * * * *",
            "0 3 * * MON-FRI",
            "30 2 1,15 * *",
            "@daily",
            "@hourly",
        ] {
            assert!(good.parse::<CronSchedule>().is_ok(), "{good}");
        }
        for bad in [
            "",
            "* * * *",
            "0 0 3 * * *",
            "61 * * * *",
            "0 25 * * *",
            "every day",
        ] {
            assert!(bad.parse::<CronSchedule>().is_err(), "{bad}");
        }
    }

    #[test]
    fn finds_the_next_time_in_utc() {
        let quarter: CronSchedule = "*/15 * * * *".parse().unwrap();
        // 10:07:30 goes to 10:15:00
        assert_eq!(quarter.next_after(NOW), Some(NOW - 450 + 900));
        let daily: CronSchedule = "@daily".parse().unwrap();
        let next = daily.next_after(NOW).unwrap();
        assert_eq!(next % 86_400, 0);
        assert!(next > NOW && next - NOW <= 86_400);
        // strictly after: a time exactly on the schedule moves to the next one
        let quarter_past = NOW - 450 + 900;
        assert_eq!(quarter.next_after(quarter_past), Some(quarter_past + 900));
    }

    #[test]
    fn timeouts_read_naturally() {
        assert_eq!("90s".parse::<CronTimeout>().unwrap().secs(), 90);
        assert_eq!("30m".parse::<CronTimeout>().unwrap().secs(), 1800);
        assert_eq!("2h".parse::<CronTimeout>().unwrap().secs(), 7200);
        assert_eq!("45".parse::<CronTimeout>().unwrap().secs(), 45);
        assert!("0s".parse::<CronTimeout>().is_err());
        assert!("25h".parse::<CronTimeout>().is_err());
        assert_eq!(CronTimeout::DEFAULT.to_string(), "1h");
        assert_eq!("90s".parse::<CronTimeout>().unwrap().to_string(), "90s");
    }

    #[test]
    fn statuses_roundtrip() {
        assert_eq!(
            "timed_out".parse::<CronRunStatus>().unwrap(),
            CronRunStatus::TimedOut
        );
        assert_eq!(CronRunStatus::TimedOut.as_str(), "timed_out");
    }
}
