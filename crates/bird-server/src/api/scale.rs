use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::ErrorBody;
use bird_api::ScaleRequest;
use bird_core::Name;

use crate::state::AppState;
use crate::{Error, Result};

/// Set the number of machines
#[utoipa::path(put, path = "/v1/services/{name}/scale", tag = "services", params(("name" = String, Path, description = "Service name")), request_body = ScaleRequest, responses((status = 202, description = "Replica count saved, machines converge in the background"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 409, description = "Services with volumes run a single machine", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn update(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(request): Json<ScaleRequest>,
) -> Result<StatusCode> {
    let service = state.service(&name).await?;
    let service_id = service.id;
    let volumes = state
        .db
        .call(move |store| store.list_volumes(service_id))
        .await?;
    if !volumes.is_empty() && request.replicas.get() > 1 {
        return Err(Error::VolumeNeedsSingleMachine(name));
    }
    state
        .db
        .call(move |store| store.set_replicas(service_id, request.replicas))
        .await?;
    tracing::info!(service = %name, replicas = %request.replicas, "scaling");
    state.reconcile_now.notify_one();
    Ok(StatusCode::ACCEPTED)
}
