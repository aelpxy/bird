use bird_core::MachineId;
use serde::{Deserialize, Serialize};

use crate::{DeployResponse, ErrorBody};

/// Where a line of a deploy's log came from
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DeployLogKind {
    /// What birdd itself is doing
    Bird,
    /// The image build, for deploys from source
    Build,
    /// The release command
    Release,
    /// A new machine, until it passes its health check
    App,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeployLogLine {
    /// Counts from 0 within the deploy, so a follower can tell what it has seen
    pub seq: u32,
    /// Unix milliseconds
    pub at: i64,
    pub kind: DeployLogKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<MachineId>,
    pub text: String,
}

/// One line of the newline-delimited json a deploy answers with when asked for
/// `application/x-ndjson`; it ends with deployed or failed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DeployEvent {
    Log(DeployLogLine),
    Deployed(DeployResponse),
    Failed(ErrorBody),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct DeploymentLog {
    pub lines: Vec<DeployLogLine>,
    /// False while the deploy is still running
    pub done: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_tagged_and_flat() {
        let event = DeployEvent::Log(DeployLogLine {
            seq: 3,
            at: 10,
            kind: DeployLogKind::Release,
            machine: None,
            text: "migrating".to_owned(),
        });
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(
            json,
            r#"{"type":"log","seq":3,"at":10,"kind":"release","text":"migrating"}"#
        );
        assert_eq!(serde_json::from_str::<DeployEvent>(&json).unwrap(), event);
        let failed = DeployEvent::Failed(ErrorBody {
            error: "nope".to_owned(),
            logs: Vec::new(),
        });
        assert_eq!(
            serde_json::to_string(&failed).unwrap(),
            r#"{"type":"failed","error":"nope"}"#
        );
    }
}
