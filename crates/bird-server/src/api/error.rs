use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use bird_api::ErrorBody;

use crate::Error;

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        let status = status_of(&self);
        if status.is_server_error() {
            tracing::error!(error = %self, "request failed");
        }
        let logs = match &self {
            Error::Unhealthy { logs, .. } => logs.clone(),
            _ => Vec::new(),
        };
        let body = ErrorBody {
            error: self.to_string(),
            logs,
        };
        (status, Json(body)).into_response()
    }
}

fn status_of(error: &Error) -> StatusCode {
    match error {
        Error::Store(bird_store::Error::AlreadyExists(_))
        | Error::Busy(_)
        | Error::DomainTaken(_)
        | Error::NotRollbackable { .. }
        | Error::VolumeNeedsSingleMachine(_)
        | Error::VolumePathChanged { .. }
        | Error::ImageChange { .. }
        | Error::HasVolumes(_)
        | Error::HasDependents(..)
        | Error::VolumeMissing(_)
        | Error::ServiceExists(_) => StatusCode::CONFLICT,
        Error::Store(bird_store::Error::NotFound(_))
        | Error::ServiceNotFound(_)
        | Error::DomainNotFound(_)
        | Error::VariableNotFound(..)
        | Error::DeploymentNotFound(_)
        | Error::NoRollbackTarget(_)
        | Error::TemplateNotFound(_)
        | Error::NoMachines(_)
        | Error::LocalImageMissing(_) => StatusCode::NOT_FOUND,
        Error::Podman(bird_podman::Error::Pull { .. })
        | Error::Validation(_)
        | Error::EmptyCredentials
        | Error::Reference(_) => StatusCode::BAD_REQUEST,
        Error::ContextTooLarge(_) => StatusCode::PAYLOAD_TOO_LARGE,
        Error::Unhealthy { .. } => StatusCode::BAD_GATEWAY,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_errors_to_statuses() {
        let name = "web".parse().unwrap();
        assert_eq!(status_of(&Error::Busy(name)), StatusCode::CONFLICT);
        assert_eq!(
            status_of(&Error::Store(bird_store::Error::AlreadyExists("domain"))),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_of(&Error::Unhealthy {
                reason: String::new(),
                logs: Vec::new()
            }),
            StatusCode::BAD_GATEWAY
        );
        assert_eq!(
            status_of(&Error::NotRollbackable {
                id: bird_core::DeploymentId::generate(),
                status: bird_core::DeploymentStatus::Failed
            }),
            StatusCode::CONFLICT
        );
        assert_eq!(
            status_of(&Error::DbClosed),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
