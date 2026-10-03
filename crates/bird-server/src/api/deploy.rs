use axum::Json;
use axum::extract::State;
use bird_api::ErrorBody;
use bird_api::{DeployRequest, DeployResponse};

use crate::state::AppState;
use crate::{Result, deploy};

#[utoipa::path(post, path = "/v1/deploy", tag = "deployments", request_body = DeployRequest, responses((status = 200, description = "Deployed and serving traffic", body = DeployResponse), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody), (status = 502, description = "The app did not become healthy; includes its last log lines", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Json(request): Json<DeployRequest>,
) -> Result<Json<DeployResponse>> {
    // run detached so a disconnecting client cannot abort a deploy halfway through
    let task = tokio::spawn(async move { deploy::deploy(&state, request).await });
    match task.await {
        Ok(result) => result.map(Json),
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}
