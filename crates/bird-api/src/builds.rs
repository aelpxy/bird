use bird_core::ImageRef;
use serde::{Deserialize, Serialize};

// one line of the newline-delimited json stream a build answers with; it ends with built or failed
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BuildEvent {
    Log { line: String },
    Built { image: ImageRef },
    Failed { error: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_are_tagged() {
        let event = BuildEvent::Built {
            image: "localhost/bird/web:1".parse().unwrap(),
        };
        let json = serde_json::to_string(&event).unwrap();
        assert_eq!(json, r#"{"type":"built","image":"localhost/bird/web:1"}"#);
        assert_eq!(serde_json::from_str::<BuildEvent>(&json).unwrap(), event);
    }
}
