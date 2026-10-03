use std::collections::BTreeMap;

use bird_core::{DeploymentId, EnvKey, Hostname, ImageRef, Name, Port};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployRequest {
    pub name: Name,
    pub image: ImageRef,
    pub port: Port,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub domain: Option<Hostname>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub env: BTreeMap<EnvKey, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeployResponse {
    pub service: Name,
    pub deployment_id: DeploymentId,
    pub image: ImageRef,
    pub domains: Vec<Hostname>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_fields() {
        let bad_name = r#"{"name":"Web","image":"nginx","port":80}"#;
        assert!(serde_json::from_str::<DeployRequest>(bad_name).is_err());
        let bad_port = r#"{"name":"web","image":"nginx","port":0}"#;
        assert!(serde_json::from_str::<DeployRequest>(bad_port).is_err());
        let bad_env = r#"{"name":"web","image":"nginx","port":80,"env":{"A-B":"1"}}"#;
        assert!(serde_json::from_str::<DeployRequest>(bad_env).is_err());
    }

    #[test]
    fn optional_fields_default() {
        let minimal = r#"{"name":"web","image":"nginx","port":80}"#;
        let request: DeployRequest = serde_json::from_str(minimal).unwrap();
        assert_eq!(request.domain, None);
        assert!(request.env.is_empty());
    }
}
