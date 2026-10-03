use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ErrorBody {
    pub error: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub logs: Vec<String>,
}
