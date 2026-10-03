use bird_core::Replicas;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleRequest {
    pub replicas: Replicas,
}
