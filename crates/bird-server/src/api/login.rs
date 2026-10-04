use std::net::SocketAddr;

use axum::extract::{ConnectInfo, State};
use axum::http::header::USER_AGENT;
use axum::http::{HeaderMap, StatusCode};
use axum::{Extension, Json};
use bird_api::{ErrorBody, LoginRequest, LoginResponse};

use super::auth::CurrentSession;
use crate::login::{self, Attempt, Outcome};
use crate::state::AppState;
use crate::{Error, Result};

// longer would only let a client fill the sessions table with junk
const MAX_AGENT_CHARS: usize = 200;

/// Sign in with a username and password, and a code when two-factor is on
#[utoipa::path(post, path = "/v1/login", tag = "users", security(()), request_body = LoginRequest, responses((status = 200, description = "Signed in, or asked for a two-factor code", body = LoginResponse), (status = 401, description = "Wrong username, password or code", body = ErrorBody), (status = 429, description = "Too many failed sign-ins, wait before trying again", body = ErrorBody)))]
pub(crate) async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>> {
    let agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|agent| agent.chars().take(MAX_AGENT_CHARS).collect());
    let attempt = Attempt {
        username: request.username,
        password: request.password,
        code: request.code,
        address: Some(peer.ip().to_string()),
        agent,
    };
    Ok(Json(match login::sign_in(&state, attempt).await? {
        Outcome::SignedIn {
            token,
            session,
            user,
        } => LoginResponse::SignedIn {
            token,
            user: user.name,
            expires_at: session.expires_at,
        },
        Outcome::TwoFactorRequired => LoginResponse::TwoFactorRequired,
    }))
}

/// Sign out the session this request came in with
#[utoipa::path(post, path = "/v1/logout", tag = "users", responses((status = 204, description = "Signed out"), (status = 400, description = "The request used an api token, not a session", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn logout(
    State(state): State<AppState>,
    Extension(principal): Extension<super::auth::Principal>,
    session: Option<Extension<CurrentSession>>,
) -> Result<StatusCode> {
    let (Some(Extension(CurrentSession(id))), super::auth::Principal::User(user)) =
        (session, principal)
    else {
        return Err(Error::NotASession);
    };
    let user_id = user.id;
    state
        .db
        .call(move |store| store.delete_session(user_id, id))
        .await?;
    tracing::info!(user = %user.name, "signed out");
    Ok(StatusCode::NO_CONTENT)
}
