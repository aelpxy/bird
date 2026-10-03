use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::ValidationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct Replicas(u8);

impl Replicas {
    pub const ONE: Self = Self(1);
    pub const MAX: u8 = 32;

    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }
}

impl Default for Replicas {
    fn default() -> Self {
        Self::ONE
    }
}

impl TryFrom<u8> for Replicas {
    type Error = ValidationError;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if (1..=Self::MAX).contains(&value) {
            Ok(Self(value))
        } else {
            Err(ValidationError::Replicas(value))
        }
    }
}

impl FromStr for Replicas {
    type Err = ValidationError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let value = s.parse::<u8>().map_err(|_| ValidationError::Replicas(0))?;
        Self::try_from(value)
    }
}

impl From<Replicas> for u8 {
    fn from(value: Replicas) -> Self {
        value.0
    }
}

impl fmt::Display for Replicas {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_one_to_max() {
        assert_eq!("1".parse::<Replicas>().unwrap(), Replicas::ONE);
        assert_eq!("32".parse::<Replicas>().unwrap().get(), 32);
        for bad in ["0", "33", "-1", "many", ""] {
            assert!(bad.parse::<Replicas>().is_err(), "{bad}");
        }
    }
}
