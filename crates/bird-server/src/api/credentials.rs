use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use bird_api::{
    DisableTwoFactor, ErrorBody, RecoveryCodes, SetPassword, TwoFactorCode, TwoFactorSetup,
};
use bird_core::{Name, User};
use serde::Deserialize;

use super::auth::{CurrentSession, Principal};
use super::users::owner;
use crate::state::AppState;
use crate::{Error, Result, credentials};

#[derive(Deserialize)]
pub(crate) struct UserPath {
    user: Name,
}

/// Set a password: your own, with the current one once you have one, or anyone's for admins
#[utoipa::path(put, path = "/v1/users/{user}/password", tag = "users", params(("user" = String, Path, description = "User name")), request_body = SetPassword, responses((status = 204, description = "Password set; the user's other sessions are signed out"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user, or the current password is wrong", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn set_password(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    session: Option<Extension<CurrentSession>>,
    Path(UserPath { user }): Path<UserPath>,
    Json(request): Json<SetPassword>,
) -> Result<StatusCode> {
    let user = owner(&state, &principal, &user).await?;
    let (current, keep) = if is_self(&principal, &user) {
        // only someone who knows the old password changes it, and their own session stays
        let current = match (user.has_password, request.current) {
            (true, None) => return Err(Error::WrongPassword),
            (_, current) => current.filter(|_| user.has_password),
        };
        (current, session.map(|Extension(CurrentSession(id))| id))
    } else {
        (None, None)
    };
    credentials::set_password(&state, &user, current, request.new, keep).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Start turning on two-factor for yourself: a secret for the authenticator app
#[utoipa::path(post, path = "/v1/users/{user}/two-factor", tag = "users", params(("user" = String, Path, description = "User name")), responses((status = 200, description = "The secret and its otpauth uri", body = TwoFactorSetup), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody)))]
pub(crate) async fn start_two_factor(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(UserPath { user }): Path<UserPath>,
) -> Result<Json<TwoFactorSetup>> {
    let user = yourself(&state, &principal, &user).await?;
    let (secret, uri) = credentials::start_two_factor(&state, &user).await?;
    Ok(Json(TwoFactorSetup { secret, uri }))
}

/// Finish turning on two-factor with a code from the app; returns the recovery codes
#[utoipa::path(post, path = "/v1/users/{user}/two-factor/confirm", tag = "users", params(("user" = String, Path, description = "User name")), request_body = TwoFactorCode, responses((status = 200, description = "Two-factor is on; recovery codes, shown only this once", body = RecoveryCodes), (status = 401, description = "Wrong code, or missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 409, description = "No setup was started", body = ErrorBody)))]
pub(crate) async fn confirm_two_factor(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(UserPath { user }): Path<UserPath>,
    Json(request): Json<TwoFactorCode>,
) -> Result<Json<RecoveryCodes>> {
    let user = yourself(&state, &principal, &user).await?;
    let codes = credentials::confirm_two_factor(&state, &user, &request.code).await?;
    Ok(Json(RecoveryCodes { codes }))
}

/// Turn off two-factor: your own with your password, or anyone's for admins
#[utoipa::path(post, path = "/v1/users/{user}/two-factor/disable", tag = "users", params(("user" = String, Path, description = "User name")), request_body = DisableTwoFactor, responses((status = 204, description = "Two-factor is off"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user, or the password is wrong", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn disable_two_factor(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(UserPath { user }): Path<UserPath>,
    Json(request): Json<DisableTwoFactor>,
) -> Result<StatusCode> {
    let user = owner(&state, &principal, &user).await?;
    if is_self(&principal, &user) && user.has_password {
        let password = request.password.ok_or(Error::WrongPassword)?;
        credentials::check_password(&state, &user, password).await?;
    }
    credentials::disable_two_factor(&state, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn is_self(principal: &Principal, user: &User) -> bool {
    matches!(principal, Principal::User(me) if me.id == user.id)
}

// two-factor is set up by its owner only, who holds the authenticator
async fn yourself(state: &AppState, principal: &Principal, name: &Name) -> Result<User> {
    let user = owner(state, principal, name).await?;
    if is_self(principal, &user) {
        Ok(user)
    } else {
        Err(Error::Forbidden(
            "only the user themselves can set up two-factor",
        ))
    }
}
