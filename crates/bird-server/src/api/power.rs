use std::future::Future;

use axum::Json;
use axum::extract::{Path, State};
use bird_api::{ErrorBody, ServiceSummary};
use bird_core::Name;

use super::services::summary;
use crate::state::AppState;
use crate::{Result, deploy};

/// Stop a service, keeping its machines and data
#[utoipa::path(post, path = "/v1/services/{name}/stop", tag = "services", params(("name" = String, Path, description = "Service name")), responses((status = 200, description = "Stopped; the supervisor leaves it alone until it is started or deployed", body = ServiceSummary), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn stop(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<ServiceSummary>> {
    detached(&state, &name, |state, name| async move {
        deploy::stop(&state, &name).await
    })
    .await
}

/// Start a stopped service
#[utoipa::path(post, path = "/v1/services/{name}/start", tag = "services", params(("name" = String, Path, description = "Service name")), responses((status = 200, description = "Its stopped machines passed their health check; missing replicas follow from the supervisor", body = ServiceSummary), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody), (status = 502, description = "A machine did not pass its health check", body = ErrorBody)))]
pub(crate) async fn start(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<ServiceSummary>> {
    detached(&state, &name, |state, name| async move {
        deploy::start(&state, &name).await
    })
    .await
}

/// Restart a service's machines one at a time
#[utoipa::path(post, path = "/v1/services/{name}/restart", tag = "services", params(("name" = String, Path, description = "Service name")), responses((status = 200, description = "Every running machine restarted and passed its health check", body = ServiceSummary), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody), (status = 502, description = "A machine did not pass its health check", body = ErrorBody)))]
pub(crate) async fn restart(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<ServiceSummary>> {
    detached(&state, &name, |state, name| async move {
        deploy::restart(&state, &name).await
    })
    .await
}

// runs detached so a disconnecting client cannot leave machines half stopped or started
async fn detached<F, Fut>(state: &AppState, name: &Name, work: F) -> Result<Json<ServiceSummary>>
where
    F: FnOnce(AppState, Name) -> Fut,
    Fut: Future<Output = Result<()>> + Send + 'static,
{
    let task = tokio::spawn(work(state.clone(), name.clone()));
    match task.await {
        Ok(result) => result?,
        Err(err) => return Err(std::io::Error::other(err).into()),
    }
    Ok(Json(summary(state, name).await?))
}
