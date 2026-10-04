mod access;
mod auth;
mod backups;
mod builds;
mod commands;
mod credentials;
mod deploy;
mod deployments;
mod docs;
mod domains;
mod error;
mod login;
mod logs;
mod openapi;
mod orgs;
mod power;
mod projects;
mod registries;
mod scale;
mod scope;
mod services;
mod sessions;
mod stream;
mod templates;
mod terminals;
mod users;
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
    let (protected, mut spec) = documented_routes().split_for_parts();
    let (public, public_spec) = public_routes().split_for_parts();
    spec.merge(public_spec);
    let spec: Arc<str> = openapi::render(&spec).into();
    let auth = auth::Auth {
        root: token,
        db: state.db.clone(),
    };
    protected
        .layer(middleware::from_fn_with_state(auth, auth::authenticate))
        .merge(public)
        .layer(DefaultBodyLimit::max(MAX_BODY_BYTES))
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

// reachable without a token, which is what signing in needs
fn public_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(login::login))
}

#[cfg(test)]
fn spec() -> utoipa::openapi::OpenApi {
    let (_, mut spec) = documented_routes().split_for_parts();
    spec.merge(public_routes().split_for_parts().1);
    spec
}

fn documented_routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::with_openapi(openapi::document())
        .routes(routes!(login::logout))
        .routes(routes!(credentials::set_password))
        .routes(routes!(credentials::start_two_factor))
        .routes(routes!(credentials::confirm_two_factor))
        .routes(routes!(credentials::disable_two_factor))
        .routes(routes!(sessions::list))
        .routes(routes!(sessions::remove))
        .routes(routes!(users::whoami))
        .routes(routes!(users::list, users::create))
        .routes(routes!(users::remove))
        .routes(routes!(users::list_tokens, users::create_token))
        .routes(routes!(users::remove_token))
        .routes(routes!(orgs::list, orgs::create))
        .routes(routes!(orgs::remove))
        .routes(routes!(orgs::members))
        .routes(routes!(orgs::set_member, orgs::remove_member))
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
