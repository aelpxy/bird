use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use bird_api::{
    CreateToken, CreateUser, CreatedUser, ErrorBody, IssuedToken, TokenSummary, UserSummary, Whoami,
};
use bird_core::{ApiToken, Name, User};
use serde::Deserialize;

use super::auth::Principal;
use crate::Result;
use crate::state::AppState;
use crate::users::{self, Issued};

#[derive(Deserialize)]
pub(crate) struct UserPath {
    user: Name,
}

#[derive(Deserialize)]
pub(crate) struct TokenPath {
    user: Name,
    token: Name,
}

/// Show who the token belongs to
#[utoipa::path(get, path = "/v1/me", tag = "users", responses((status = 200, description = "The token's user and role", body = Whoami), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn whoami(Extension(principal): Extension<Principal>) -> Json<Whoami> {
    Json(Whoami {
        name: principal.name().to_owned(),
        role: principal.role(),
    })
}

/// List users, for admins
#[utoipa::path(get, path = "/v1/users", tag = "users", responses((status = 200, description = "Every user", body = Vec<UserSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<UserSummary>>> {
    principal.require_admin("only server admins can manage users")?;
    Ok(Json(
        users::list(&state)
            .await?
            .into_iter()
            .map(summary)
            .collect(),
    ))
}

/// Create a user and a first token for them, for admins
#[utoipa::path(post, path = "/v1/users", tag = "users", request_body = CreateUser, responses((status = 201, description = "The user and their first token, shown only this once", body = CreatedUser), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(request): Json<CreateUser>,
) -> Result<(StatusCode, Json<CreatedUser>)> {
    principal.require_admin("only server admins can manage users")?;
    let (user, issued) = users::create(&state, &request.name, request.role).await?;
    let created = CreatedUser {
        user: summary(user),
        token: issued_token(issued),
    };
    Ok((StatusCode::CREATED, Json(created)))
}

/// Delete a user and their tokens, for admins
#[utoipa::path(delete, path = "/v1/users/{user}", tag = "users", params(("user" = String, Path, description = "User name")), responses((status = 204, description = "User deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(UserPath { user }): Path<UserPath>,
) -> Result<StatusCode> {
    principal.require_admin("only server admins can manage users")?;
    users::remove(&state, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List a user's tokens, your own or anyone's for admins
#[utoipa::path(get, path = "/v1/users/{user}/tokens", tag = "users", params(("user" = String, Path, description = "User name")), responses((status = 200, description = "The user's tokens, without their secrets", body = Vec<TokenSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn list_tokens(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(UserPath { user }): Path<UserPath>,
) -> Result<Json<Vec<TokenSummary>>> {
    let user = owner(&state, &principal, &user).await?;
    let tokens = users::tokens(&state, &user).await?;
    Ok(Json(tokens.into_iter().map(token_summary).collect()))
}

/// Create a token, for yourself or anyone for admins
#[utoipa::path(post, path = "/v1/users/{user}/tokens", tag = "users", params(("user" = String, Path, description = "User name")), request_body = CreateToken, responses((status = 201, description = "The token, shown only this once", body = IssuedToken), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn create_token(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(UserPath { user }): Path<UserPath>,
    Json(request): Json<CreateToken>,
) -> Result<(StatusCode, Json<IssuedToken>)> {
    let user = owner(&state, &principal, &user).await?;
    let issued = users::issue(&state, &user, &request.name, request.expires_in_days).await?;
    Ok((StatusCode::CREATED, Json(issued_token(issued))))
}

/// Delete a token, yours or anyone's for admins
#[utoipa::path(delete, path = "/v1/users/{user}/tokens/{token}", tag = "users", params(("user" = String, Path, description = "User name"), ("token" = String, Path, description = "Token name")), responses((status = 204, description = "Token deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn remove_token(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(TokenPath { user, token }): Path<TokenPath>,
) -> Result<StatusCode> {
    let user = owner(&state, &principal, &user).await?;
    users::revoke(&state, &user, &token).await?;
    Ok(StatusCode::NO_CONTENT)
}

// checked before the lookup, so members cannot learn which users exist
async fn owner(state: &AppState, principal: &Principal, name: &Name) -> Result<User> {
    principal.require_self_or_admin(name)?;
    users::find(state, name).await
}

fn summary(user: User) -> UserSummary {
    UserSummary {
        name: user.name,
        role: user.role,
        created_at: user.created_at,
    }
}

fn token_summary(token: ApiToken) -> TokenSummary {
    TokenSummary {
        name: token.name,
        prefix: token.prefix,
        created_at: token.created_at,
        last_used_at: token.last_used_at,
        expires_at: token.expires_at,
    }
}

fn issued_token(issued: Issued) -> IssuedToken {
    IssuedToken {
        name: issued.token.name,
        token: issued.secret,
        expires_at: issued.token.expires_at,
    }
}
