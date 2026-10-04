mod auth;
mod backups;
mod builds;
mod commands;
mod deploy;
mod deployments;
mod docs;
mod domains;
mod error;
mod logs;
mod openapi;
mod power;
mod projects;
mod registries;
mod scale;
mod scope;
mod services;
mod stream;
mod templates;
mod terminals;
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
        .routes(routes!(projects::list, projects::create))
        .routes(routes!(projects::remove))
        .routes(routes!(projects::create_environment))
        .routes(routes!(projects::remove_environment))
        .routes(routes!(deploy::create))
        .routes(routes!(builds::create))
        .routes(routes!(services::list))
        .routes(routes!(services::get, services::remove))
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
        .routes(routes!(backups::create, backups::list))
        .routes(routes!(backups::restore))
        .routes(routes!(backups::remove))
        .routes(routes!(backups::set_schedule, backups::clear_schedule))
        .routes(routes!(commands::exec))
        .routes(routes!(commands::run))
        .routes(routes!(terminals::exec))
        .routes(routes!(terminals::run))
        .routes(routes!(terminals::exec_pipe))
        .routes(routes!(terminals::run_pipe))
        .routes(routes!(power::stop))
        .routes(routes!(power::start))
        .routes(routes!(power::restart))
}
