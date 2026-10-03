mod auth;
mod deploy;
mod deployments;
mod domains;
mod error;
mod logs;
mod scale;
mod services;
mod stream;
mod variables;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::middleware;
use axum::routing::{delete, get, post, put};

use crate::state::AppState;
use crate::token::ApiToken;

const MAX_BODY_BYTES: usize = 64 * 1024;

pub(crate) fn router(state: AppState, token: ApiToken) -> Router {
    Router::new()
        .route("/v1/deploy", post(deploy::create))
        .route("/v1/services", get(services::list))
        .route("/v1/services/{name}", delete(services::remove))
        .route(
            "/v1/services/{name}/domains",
            get(domains::list).post(domains::add),
        )
        .route(
            "/v1/services/{name}/domains/{hostname}",
            delete(domains::remove),
        )
        .route("/v1/services/{name}/deployments", get(deployments::list))
        .route("/v1/services/{name}/rollback", post(deployments::rollback))
        .route("/v1/services/{name}/logs", get(logs::logs))
        .route("/v1/services/{name}/scale", put(scale::update))
        .route(
            "/v1/services/{name}/variables",
            get(variables::list).patch(variables::update),
        )
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(token, auth::require_token))
        .with_state(state)
}
