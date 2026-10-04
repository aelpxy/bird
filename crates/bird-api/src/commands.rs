use bird_core::Command;
use serde::{Deserialize, Serialize};

use crate::LogStream;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct ExecRequest {
    pub command: Command,
    /// Machine id, or the end of one as `bird status` shows it; left out, the first running machine
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
pub struct RunRequest {
    pub command: Command,
}

// one line of the newline-delimited json stream exec and run answer with; it ends with exited or failed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandEvent {
    /// Output exactly as written, so a chunk may end mid-line; invalid UTF-8 becomes U+FFFD
    Output {
        stream: LogStream,
        text: String,
    },
    Exited {
        code: i32,
    },
    Failed {
        error: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_tagged() {
        let event = CommandEvent::Output {
            stream: LogStream::Stderr,
            text: "oops".to_owned(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"type":"output","stream":"stderr","text":"oops"}"#);
        assert_eq!(serde_json::from_str::<CommandEvent>(&json).unwrap(), event);
        let exited = serde_json::to_string(&CommandEvent::Exited { code: 3 }).unwrap();
        assert_eq!(exited, r#"{"type":"exited","code":3}"#);
    }

    #[test]
    fn requests_need_a_command() {
        let request: ExecRequest =
            serde_json::from_str(r#"{"command":["psql","-c","select 1"]}"#).unwrap();
        assert_eq!(request.machine, None);
        assert!(serde_json::from_str::<RunRequest>(r#"{"command":[]}"#).is_err());
    }
}
