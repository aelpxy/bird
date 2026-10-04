use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use bird_api::{ErrorBody, SessionSummary};
use bird_core::{Name, SessionId};
use serde::Deserialize;

use super::auth::{CurrentSession, Principal};
use super::users::owner;
use crate::state::AppState;
use crate::{Error, Result};

#[derive(Deserialize)]
pub(crate) struct UserPath {
    user: Name,
}

#[derive(Deserialize)]
pub(crate) struct SessionPath {
    user: Name,
    id: SessionId,
}

/// List where a user is signed in: your own, or anyone's for admins
#[utoipa::path(get, path = "/v1/users/{user}/sessions", tag = "users", params(("user" = String, Path, description = "User name")), responses((status = 200, description = "Sessions that have not expired, newest use first", body = Vec<SessionSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    current: Option<Extension<CurrentSession>>,
    Path(UserPath { user }): Path<UserPath>,
) -> Result<Json<Vec<SessionSummary>>> {
    let user = owner(&state, &principal, &user).await?;
    let current = current.map(|Extension(CurrentSession(id))| id);
    let user_id = user.id;
    let sessions = state
        .db
        .call(move |store| store.list_sessions(user_id))
        .await?;
    Ok(Json(
        sessions
            .into_iter()
            .map(|session| SessionSummary {
                current: Some(session.id) == current,
                id: session.id,
                created_at: session.created_at,
                last_used_at: session.last_used_at,
                expires_at: session.expires_at,
                address: session.address,
                agent: session.agent,
            })
            .collect(),
    ))
}

/// Sign a session out: your own, or anyone's for admins
#[utoipa::path(delete, path = "/v1/users/{user}/sessions/{id}", tag = "users", params(("user" = String, Path, description = "User name"), ("id" = String, Path, description = "Session id")), responses((status = 204, description = "Signed out"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(SessionPath { user, id }): Path<SessionPath>,
) -> Result<StatusCode> {
    let user = owner(&state, &principal, &user).await?;
    let user_id = user.id;
    state
        .db
        .call(move |store| store.delete_session(user_id, id))
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::NotFound(_)) => Error::SessionNotFound(id),
            other => other,
        })?;
    tracing::info!(user = %user.name, session = %id, "session signed out");
    Ok(StatusCode::NO_CONTENT)
}
