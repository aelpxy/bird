mod auth;
mod deploy;
mod deployments;
mod docs;
mod domains;
mod error;
mod logs;
mod openapi;
mod registries;
mod scale;
mod services;
mod stream;
mod templates;
mod variables;

use std::sync::Arc;

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::header::CONTENT_TYPE;
use axum::middleware;
use axum::routing::get;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::state::AppState;
use crate::token::ApiToken;

const MAX_BODY_BYTES: usize = 64 * 1024;

pub(crate) fn router(state: AppState, token: ApiToken) -> Router {
    let (api, spec) = documented_routes().split_for_parts();
    let spec: Arc<str> = openapi::render(&spec).into();
    api.layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
        .layer(middleware::from_fn_with_state(token, auth::require_token))
        .route("/docs", get(docs::page))
        .route(
            "/v1/openapi.json",
            get(move || {
                let spec = Arc::clone(&spec);
                async move { ([(CONTENT_TYPE, "application/json")], spec.to_string()) }
            }),
        )
        .with_state(state)
}

fn documented_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(openapi::document())
        .routes(routes!(deploy::create))
        .routes(routes!(services::list))
        .routes(routes!(services::remove))
        .routes(routes!(domains::list, domains::add))
        .routes(routes!(domains::remove))
        .routes(routes!(deployments::list))
        .routes(routes!(deployments::rollback))
        .routes(routes!(logs::logs))
        .routes(routes!(scale::update))
        .routes(routes!(variables::list, variables::update))
        .routes(routes!(variables::get))
        .routes(routes!(registries::list))
        .routes(routes!(registries::login, registries::logout))
        .routes(routes!(templates::list))
        .routes(routes!(templates::deploy))
}
