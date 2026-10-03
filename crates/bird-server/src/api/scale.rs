use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::ScaleRequest;
use bird_core::Name;

use crate::Result;
use crate::state::AppState;

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(request): Json<ScaleRequest>,
) -> Result<StatusCode> {
    let service = state.service(&name).await?;
    let service_id = service.id;
    state
        .db
        .call(move |store| store.set_replicas(service_id, request.replicas))
        .await?;
    tracing::info!(service = %name, replicas = %request.replicas, "scaling");
    state.reconcile_now.notify_one();
    Ok(StatusCode::ACCEPTED)
}
