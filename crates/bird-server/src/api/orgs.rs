use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use bird_api::{CreateOrg, ErrorBody, OrgMember, OrgSummary, SetMember};
use bird_core::{Name, Org, OrgRole, UserRole};
use serde::Deserialize;

use super::access::{forbidden, role_in};
use super::auth::Principal;
use crate::state::AppState;
use crate::{Error, Result, orgs, users};

#[derive(Deserialize)]
pub(crate) struct OrgPath {
    org: Name,
}

#[derive(Deserialize)]
pub(crate) struct MemberPath {
    org: Name,
    user: Name,
}

/// List your orgs, or every org for server admins
#[utoipa::path(get, path = "/v1/orgs", tag = "orgs", responses((status = 200, description = "Orgs with your role in each", body = Vec<OrgSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<OrgSummary>>> {
    let joined: HashMap<_, _> = match &principal {
        Principal::Root => HashMap::new(),
        Principal::User(user) => {
            let user_id = user.id;
            state
                .db
                .call(move |store| store.user_orgs(user_id))
                .await?
                .into_iter()
                .map(|(org, role)| (org.id, role))
                .collect()
        }
    };
    let every = principal.role() == UserRole::Admin;
    let all = state.db.call(|store| store.list_orgs()).await?;
    let visible = all
        .into_iter()
        .filter(|org| every || joined.contains_key(&org.id))
        .map(|org| OrgSummary {
            role: joined.get(&org.id).copied(),
            name: org.name,
        })
        .collect();
    Ok(Json(visible))
}

/// Create an org, for server admins; a user who creates one becomes its owner
#[utoipa::path(post, path = "/v1/orgs", tag = "orgs", request_body = CreateOrg, responses((status = 201, description = "Org created", body = OrgSummary), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(request): Json<CreateOrg>,
) -> Result<(StatusCode, Json<OrgSummary>)> {
    principal.require_admin("only server admins can create orgs")?;
    let owner = match &principal {
        Principal::Root => None,
        Principal::User(user) => Some(user),
    };
    let org = orgs::create(&state, &request.name, owner).await?;
    let created = OrgSummary {
        name: org.name,
        role: owner.map(|_| OrgRole::Owner),
    };
    Ok((StatusCode::CREATED, Json(created)))
}

/// Delete an org that owns no projects, for its owners
#[utoipa::path(delete, path = "/v1/orgs/{org}", tag = "orgs", params(("org" = String, Path, description = "Org name")), responses((status = 204, description = "Org deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(OrgPath { org }): Path<OrgPath>,
) -> Result<StatusCode> {
    let org = visible(&state, &principal, &org, OrgRole::Owner).await?;
    orgs::remove(&state, &org).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// List an org's members, for its members
#[utoipa::path(get, path = "/v1/orgs/{org}/members", tag = "orgs", params(("org" = String, Path, description = "Org name")), responses((status = 200, description = "Members and their roles", body = Vec<OrgMember>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn members(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(OrgPath { org }): Path<OrgPath>,
) -> Result<Json<Vec<OrgMember>>> {
    let org = visible(&state, &principal, &org, OrgRole::Member).await?;
    let members = orgs::members(&state, &org).await?;
    Ok(Json(
        members
            .into_iter()
            .map(|(user, role)| OrgMember {
                user: user.name,
                role,
            })
            .collect(),
    ))
}

/// Add a user to an org or change their role, for its owners
#[utoipa::path(put, path = "/v1/orgs/{org}/members/{user}", tag = "orgs", params(("org" = String, Path, description = "Org name"), ("user" = String, Path, description = "User name")), request_body = SetMember, responses((status = 204, description = "Member set"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn set_member(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(MemberPath { org, user }): Path<MemberPath>,
    Json(request): Json<SetMember>,
) -> Result<StatusCode> {
    let org = visible(&state, &principal, &org, OrgRole::Owner).await?;
    let user = users::find(&state, &user).await?;
    orgs::set_member(&state, &org, &user, request.role).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Remove a user from an org, for its owners or the user leaving
#[utoipa::path(delete, path = "/v1/orgs/{org}/members/{user}", tag = "orgs", params(("org" = String, Path, description = "Org name"), ("user" = String, Path, description = "User name")), responses((status = 204, description = "Member removed"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove_member(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(MemberPath { org, user }): Path<MemberPath>,
) -> Result<StatusCode> {
    let leaving = principal.name() == user.as_str();
    let needed = if leaving {
        OrgRole::Member
    } else {
        OrgRole::Owner
    };
    let org = visible(&state, &principal, &org, needed).await?;
    let user = users::find(&state, &user).await?;
    orgs::remove_member(&state, &org, &user).await?;
    Ok(StatusCode::NO_CONTENT)
}

// an org the principal does not belong to looks missing, so its name does not leak
async fn visible(
    state: &AppState,
    principal: &Principal,
    name: &Name,
    needed: OrgRole,
) -> Result<Org> {
    let org = orgs::find(state, name).await?;
    match role_in(state, principal, org.id).await? {
        None => Err(Error::OrgNotFound(name.clone())),
        Some(role) if role.at_least(needed) => Ok(org),
        Some(_) => Err(forbidden(needed)),
    }
}
