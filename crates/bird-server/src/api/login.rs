use std::net::{IpAddr, SocketAddr};

use axum::extract::{ConnectInfo, State};
use axum::http::header::{SET_COOKIE, USER_AGENT};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json};
use bird_api::{ErrorBody, LoginRequest, LoginResponse};

use super::auth::CurrentSession;
use super::cookies;
use crate::login::{self, Attempt, Outcome, SESSION_LIFETIME};
use crate::state::AppState;
use crate::{Error, Result};

// longer would only let a client fill the sessions table with junk
const MAX_AGENT_CHARS: usize = 200;

/// Sign in with a username and password, and a code when two-factor is on
#[utoipa::path(post, path = "/v1/login", tag = "users", security(()), request_body = LoginRequest, responses((status = 200, description = "Signed in, or asked for a two-factor code; with `cookie` the session is set as an HttpOnly `bird_session` cookie, marked Secure unless the browser is on this host, and requests using it that change something or open a terminal must send an Origin matching the Host", body = LoginResponse), (status = 401, description = "Wrong username, password or code", body = ErrorBody), (status = 429, description = "Too many failed sign-ins, wait before trying again", body = ErrorBody)))]
pub(crate) async fn login(
    State(state): State<AppState>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    Json(request): Json<LoginRequest>,
) -> Result<Response> {
    let agent = headers
        .get(USER_AGENT)
        .and_then(|value| value.to_str().ok())
        .map(|agent| agent.chars().take(MAX_AGENT_CHARS).collect());
    // anything that did not come straight from this host may cross a network in the clear
    let secure = !peer.ip().is_loopback() || headers.contains_key("x-forwarded-for");
    let cookie = request.cookie;
    let attempt = Attempt {
        username: request.username,
        password: request.password,
        code: request.code,
        address: Some(client_address(peer.ip(), &headers).to_string()),
        agent,
    };
    Ok(match login::sign_in(&state, attempt).await? {
        Outcome::SignedIn {
            token,
            session,
            user,
        } if cookie => {
            let set = cookies::set_session(&token, SESSION_LIFETIME, secure).ok_or_else(|| {
                std::io::Error::other("the session token does not fit in a cookie")
            })?;
            let body = LoginResponse::SignedInWithCookie {
                user: user.name,
                expires_at: session.expires_at,
            };
            ([(SET_COOKIE, set)], Json(body)).into_response()
        }
        Outcome::SignedIn {
            token,
            session,
            user,
        } => Json(LoginResponse::SignedIn {
            token,
            user: user.name,
            expires_at: session.expires_at,
        })
        .into_response(),
        Outcome::TwoFactorRequired => Json(LoginResponse::TwoFactorRequired).into_response(),
    })
}

/// Sign out the session this request came in with
#[utoipa::path(post, path = "/v1/logout", tag = "users", responses((status = 204, description = "Signed out, and the session cookie cleared"), (status = 400, description = "The request used an api token, not a session", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn logout(
    State(state): State<AppState>,
    Extension(principal): Extension<super::auth::Principal>,
    session: Option<Extension<CurrentSession>>,
) -> Result<impl IntoResponse> {
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
    Ok((
        StatusCode::NO_CONTENT,
        [(SET_COOKIE, cookies::clear_session())],
    ))
}

// behind the proxy every peer is loopback, and the proxy replaces any forwarded-for a client sent
fn client_address(peer: IpAddr, headers: &HeaderMap) -> IpAddr {
    headers
        .get("x-forwarded-for")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse().ok())
        .filter(|_| peer.is_loopback())
        .unwrap_or(peer)
}
