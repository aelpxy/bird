use std::collections::BTreeMap;
use std::fmt;

use bird_core::EnvKey;
use serde::{Deserialize, Serialize};

use crate::DeployResponse;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateVariables {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub set: BTreeMap<EnvKey, String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub unset: Vec<EnvKey>,
    #[serde(default = "redeploy_by_default")]
    pub deploy: bool,
}

const fn redeploy_by_default() -> bool {
    true
}

impl fmt::Debug for UpdateVariables {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UpdateVariables")
            .field("set", &self.set.keys().collect::<Vec<_>>())
            .field("unset", &self.unset)
            .field("deploy", &self.deploy)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VariablesResponse {
    pub keys: Vec<EnvKey>,
    pub deployment: Option<DeployResponse>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deploys_by_default_and_hides_values() {
        let update: UpdateVariables =
            serde_json::from_str(r#"{"set":{"TOKEN":"hunter2"}}"#).unwrap();
        assert!(update.deploy);
        assert_eq!(update.unset, Vec::new());
        assert!(!format!("{update:?}").contains("hunter2"));
    }
}
