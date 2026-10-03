mod deploy;
mod error;
mod logs;
mod services;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::routing::{delete, get, post};

use crate::state::AppState;

const MAX_BODY_BYTES: usize = 64 * 1024;

pub(crate) fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/deploy", post(deploy::create))
        .route("/v1/services", get(services::list))
        .route("/v1/services/{name}", delete(services::remove))
        .route("/v1/services/{name}/logs", get(logs::tail))
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .with_state(state)
}
