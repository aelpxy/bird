use bird_core::Hostname;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddDomain {
    pub hostname: Hostname,
}
