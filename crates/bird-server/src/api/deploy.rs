use axum::Json;
use axum::extract::State;
use bird_api::ErrorBody;
use bird_api::{DeployRequest, DeployResponse};

use super::scope::Scope;
use crate::state::AppState;
use crate::{Result, deploy};

/// Deploy an image, creating the service on first deploy
#[utoipa::path(post, path = "/v1/projects/{project}/environments/{environment}/deploy", tag = "deployments", request_body = DeployRequest, params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name")), responses((status = 200, description = "Deployed and serving traffic", body = DeployResponse), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody), (status = 502, description = "The app did not become healthy; includes its last log lines", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Scope(environment): Scope,
    Json(request): Json<DeployRequest>,
) -> Result<Json<DeployResponse>> {
    // run detached so a disconnecting client cannot abort a deploy halfway through
    let task = tokio::spawn(async move { deploy::deploy(&state, environment, request).await });
    match task.await {
        Ok(result) => result.map(Json),
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}
