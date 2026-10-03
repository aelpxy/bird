use axum::Json;
use axum::extract::State;
use bird_api::{DeployRequest, DeployResponse};

use crate::state::AppState;
use crate::{Result, deploy};

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
