use serde::{Deserialize, Serialize};

use crate::MachineId;

const MINUTE: i64 = 60;
const HOUR: i64 = 60 * MINUTE;
const DAY: i64 = 24 * HOUR;

/// How far apart stored samples are; each rolls into the next as it ages, so a month stays small
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Resolution {
    HalfMinute,
    FiveMinutes,
    Hour,
}

impl Resolution {
    pub const ALL: [Self; 3] = [Self::HalfMinute, Self::FiveMinutes, Self::Hour];

    #[must_use]
    pub const fn seconds(self) -> i64 {
        match self {
            Self::HalfMinute => 30,
            Self::FiveMinutes => 5 * MINUTE,
            Self::Hour => HOUR,
        }
    }

    /// How long samples stay at this resolution before rolling up, or being dropped from the last
    #[must_use]
    pub const fn kept_secs(self) -> i64 {
        match self {
            Self::HalfMinute => 6 * HOUR,
            Self::FiveMinutes => 7 * DAY,
            Self::Hour => 30 * DAY,
        }
    }

    #[must_use]
    pub const fn coarser(self) -> Option<Self> {
        match self {
            Self::HalfMinute => Some(Self::FiveMinutes),
            Self::FiveMinutes => Some(Self::Hour),
            Self::Hour => None,
        }
    }
}

/// A stretch of usage history to read
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub enum MetricsRange {
    #[default]
    #[serde(rename = "1h")]
    Hour,
    #[serde(rename = "6h")]
    SixHours,
    #[serde(rename = "7d")]
    Week,
    #[serde(rename = "30d")]
    Month,
}

impl MetricsRange {
    #[must_use]
    pub const fn seconds(self) -> i64 {
        match self {
            Self::Hour => HOUR,
            Self::SixHours => 6 * HOUR,
            Self::Week => 7 * DAY,
            Self::Month => 30 * DAY,
        }
    }

    /// Seconds per point, about 120 to 170 points whatever the range
    #[must_use]
    pub const fn step_secs(self) -> i64 {
        match self {
            Self::Hour => 30,
            Self::SixHours => 3 * MINUTE,
            Self::Week => HOUR,
            Self::Month => 4 * HOUR,
        }
    }
}

/// One machine's usage over one sample, as the sampler writes it
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MetricSample {
    pub machine: MachineId,
    pub cpu_millicores: u64,
    pub memory_bytes: u64,
    /// Received during the sample
    pub net_rx_bytes: u64,
    /// Sent during the sample
    pub net_tx_bytes: u64,
}

/// A service's usage over one step: averages and peaks summed over its machines, traffic in total
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct MetricPoint {
    /// Start of the step, in unix seconds
    pub at: i64,
    pub cpu_millicores: u64,
    pub cpu_peak_millicores: u64,
    pub memory_bytes: u64,
    pub memory_peak_bytes: u64,
    pub net_rx_bytes: u64,
    pub net_tx_bytes: u64,
    /// Machines that reported during the step
    pub machines: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_resolution_rolls_into_a_coarser_one_that_divides_it() {
        for resolution in Resolution::ALL {
            if let Some(next) = resolution.coarser() {
                assert_eq!(next.seconds() % resolution.seconds(), 0);
                assert!(next.kept_secs() > resolution.kept_secs());
            }
        }
    }

    #[test]
    fn reads_ranges_as_their_short_names() {
        let range: MetricsRange = serde_json::from_str("\"7d\"").unwrap();
        assert_eq!(range, MetricsRange::Week);
        assert!(range.seconds() / range.step_secs() <= 170);
    }
}
