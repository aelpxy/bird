use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

const MIB_PER_GIB: u32 = 1024;
const MILLICORES_PER_CPU: u32 = 1000;

// memory a machine may use, in MiB; parsed from forms like 512m or 2g
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = u32, minimum = 32, maximum = 262_144))]
#[serde(try_from = "u32", into = "u32")]
pub struct MemoryLimit(u32);

impl MemoryLimit {
    pub const DEFAULT: Self = Self(1024);
    const RANGE: std::ops::RangeInclusive<u32> = 32..=262_144;

    #[must_use]
    pub const fn mebibytes(self) -> u32 {
        self.0
    }

    #[must_use]
    pub fn bytes(self) -> u64 {
        u64::from(self.0) * 1024 * 1024
    }
}

impl TryFrom<u32> for MemoryLimit {
    type Error = ValidationError;

    fn try_from(mebibytes: u32) -> Result<Self, Self::Error> {
        if Self::RANGE.contains(&mebibytes) {
            Ok(Self(mebibytes))
        } else {
            Err(ValidationError::Memory(mebibytes.to_string()))
        }
    }
}

impl FromStr for MemoryLimit {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ValidationError::Memory(s.to_owned());
        let lower = s.trim().to_ascii_lowercase();
        let (digits, scale) = match lower.strip_suffix('g') {
            Some(digits) => (digits, MIB_PER_GIB),
            None => (lower.strip_suffix('m').unwrap_or(&lower), 1),
        };
        let amount: u32 = digits.parse().map_err(|_| invalid())?;
        let mebibytes = amount.checked_mul(scale).ok_or_else(invalid)?;
        Self::try_from(mebibytes).map_err(|_| invalid())
    }
}

impl From<MemoryLimit> for u32 {
    fn from(limit: MemoryLimit) -> Self {
        limit.0
    }
}

impl fmt::Display for MemoryLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_multiple_of(MIB_PER_GIB) {
            write!(f, "{}g", self.0 / MIB_PER_GIB)
        } else {
            write!(f, "{}m", self.0)
        }
    }
}

// cpu time a machine may use, in thousandths of a core; parsed from forms like 0.5 or 2
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = u32, minimum = 100, maximum = 64_000))]
#[serde(try_from = "u32", into = "u32")]
pub struct CpuLimit(u32);

impl CpuLimit {
    pub const DEFAULT: Self = Self(MILLICORES_PER_CPU);
    const RANGE: std::ops::RangeInclusive<u32> = 100..=64_000;

    #[must_use]
    pub const fn millicores(self) -> u32 {
        self.0
    }
}

impl TryFrom<u32> for CpuLimit {
    type Error = ValidationError;

    fn try_from(millicores: u32) -> Result<Self, Self::Error> {
        if Self::RANGE.contains(&millicores) {
            Ok(Self(millicores))
        } else {
            Err(ValidationError::Cpu(millicores.to_string()))
        }
    }
}

impl FromStr for CpuLimit {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let invalid = || ValidationError::Cpu(s.to_owned());
        let (whole, fraction) = s.trim().split_once('.').unwrap_or((s.trim(), ""));
        if fraction.len() > 3 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
            return Err(invalid());
        }
        let whole: u32 = if whole.is_empty() {
            0
        } else {
            whole.parse().map_err(|_| invalid())?
        };
        let fraction: u32 = format!("{fraction:0<3}").parse().map_err(|_| invalid())?;
        let millicores = whole
            .checked_mul(MILLICORES_PER_CPU)
            .and_then(|m| m.checked_add(fraction))
            .ok_or_else(invalid)?;
        Self::try_from(millicores).map_err(|_| invalid())
    }
}

impl From<CpuLimit> for u32 {
    fn from(limit: CpuLimit) -> Self {
        limit.0
    }
}

impl fmt::Display for CpuLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let whole = self.0 / MILLICORES_PER_CPU;
        let fraction = self.0 % MILLICORES_PER_CPU;
        if fraction == 0 {
            write!(f, "{whole}")
        } else {
            let digits = format!("{fraction:03}");
            write!(f, "{whole}.{}", digits.trim_end_matches('0'))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_and_prints_memory() {
        assert_eq!("512m".parse::<MemoryLimit>().unwrap().mebibytes(), 512);
        assert_eq!("2G".parse::<MemoryLimit>().unwrap().mebibytes(), 2048);
        assert_eq!("768".parse::<MemoryLimit>().unwrap().mebibytes(), 768);
        assert_eq!("2g".parse::<MemoryLimit>().unwrap().to_string(), "2g");
        assert_eq!("1536m".parse::<MemoryLimit>().unwrap().to_string(), "1536m");
        assert_eq!(MemoryLimit::DEFAULT.bytes(), 1_073_741_824);
        for bad in ["", "16m", "1t", "-1g", "lots", "999999g"] {
            assert!(bad.parse::<MemoryLimit>().is_err(), "{bad}");
        }
    }

    #[test]
    fn parses_and_prints_cpus() {
        assert_eq!("0.5".parse::<CpuLimit>().unwrap().millicores(), 500);
        assert_eq!("2".parse::<CpuLimit>().unwrap().millicores(), 2000);
        assert_eq!(".25".parse::<CpuLimit>().unwrap().millicores(), 250);
        assert_eq!("1.5".parse::<CpuLimit>().unwrap().to_string(), "1.5");
        assert_eq!("0.25".parse::<CpuLimit>().unwrap().to_string(), "0.25");
        assert_eq!(CpuLimit::DEFAULT.to_string(), "1");
        for bad in ["", "0.05", "0.1234", "65", "x", "-1"] {
            assert!(bad.parse::<CpuLimit>().is_err(), "{bad}");
        }
    }
}
