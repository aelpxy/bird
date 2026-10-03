use std::path::PathBuf;

use bytes::Bytes;
use hyper::StatusCode;
use serde::Deserialize;

use crate::transport::Response;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("cannot connect to podman socket {path}: {source}")]
    Connect {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("podman request timed out")]
    Timeout,
    #[error("invalid podman request: {0}")]
    Request(#[from] hyper::http::Error),
    #[error("podman connection failed: {0}")]
    Http(#[from] hyper::Error),
    #[error("failed to read podman response: {0}")]
    Body(Box<dyn std::error::Error + Send + Sync>),
    #[error("invalid podman response: {0}")]
    Decode(#[from] serde_json::Error),
    #[error("{subject} not found: {message}")]
    NotFound { subject: String, message: String },
    #[error("{subject} already exists: {message}")]
    Conflict { subject: String, message: String },
    #[error("podman returned {status}: {message}")]
    Api { status: u16, message: String },
    #[error("failed to pull image {image}: {message}")]
    Pull { image: String, message: String },
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

// podman reports some duplicates as a 500 with one of these causes instead of a 409
const ALREADY_EXISTS_CAUSES: [&str; 2] = ["that name is already in use", "volume already exists"];

#[derive(Deserialize)]
struct ApiError {
    message: String,
    #[serde(default)]
    cause: String,
}

pub(crate) fn check(response: Response, subject: impl FnOnce() -> String) -> Result<Bytes> {
    let status = response.status;
    if status.is_success() || status == StatusCode::NOT_MODIFIED {
        return Ok(response.body);
    }
    let ApiError { message, cause } = parse_api_error(&response.body);
    Err(match status {
        StatusCode::NOT_FOUND => Error::NotFound {
            subject: subject(),
            message,
        },
        StatusCode::CONFLICT => Error::Conflict {
            subject: subject(),
            message,
        },
        _ if ALREADY_EXISTS_CAUSES.contains(&cause.as_str()) => Error::Conflict {
            subject: subject(),
            message,
        },
        _ => Error::Api {
            status: status.as_u16(),
            message,
        },
    })
}

fn parse_api_error(body: &[u8]) -> ApiError {
    serde_json::from_slice(body).unwrap_or_else(|_| ApiError {
        message: String::from_utf8_lossy(body).trim().to_owned(),
        cause: String::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(status: u16, body: &str) -> Response {
        Response {
            status: StatusCode::from_u16(status).unwrap(),
            body: Bytes::from(body.to_owned()),
        }
    }

    #[test]
    fn maps_statuses() {
        assert!(check(response(204, ""), String::new).is_ok());
        assert!(check(response(304, ""), String::new).is_ok());
        assert!(matches!(
            check(
                response(404, r#"{"cause":"x","message":"no such container","response":404}"#),
                || "container a".to_owned()
            ),
            Err(Error::NotFound { subject, message }) if subject == "container a" && message == "no such container"
        ));
        assert!(matches!(
            check(response(409, "{}"), String::new),
            Err(Error::Conflict { .. })
        ));
        assert!(matches!(
            check(
                response(
                    500,
                    r#"{"cause":"that name is already in use","message":"taken"}"#
                ),
                String::new
            ),
            Err(Error::Conflict { .. })
        ));
        assert!(matches!(
            check(response(500, "boom"), String::new),
            Err(Error::Api { status: 500, message }) if message == "boom"
        ));
    }
}
