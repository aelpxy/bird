use std::collections::{HashMap, HashSet};

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::{Extension, Json};
use bird_api::{CreateEnvironment, CreateProject, ErrorBody, ProjectSummary};
use bird_core::{Environment, Name, Org, OrgRole, Project, UserRole};
use serde::Deserialize;

use super::access::{forbidden, require_project, role_in};
use super::auth::Principal;
use crate::run::DEFAULT_ORG;
use crate::state::AppState;
use crate::{Error, Result, environments, orgs};

#[derive(Deserialize)]
pub(crate) struct ProjectPath {
    project: Name,
}

#[derive(Deserialize)]
pub(crate) struct EnvironmentPath {
    project: Name,
    environment: Name,
}

/// List the projects of your orgs, or every project for server admins
#[utoipa::path(get, path = "/v1/projects", tag = "projects", responses((status = 200, description = "Projects with their org and environments", body = Vec<ProjectSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
) -> Result<Json<Vec<ProjectSummary>>> {
    let projects = environments::list(&state).await?;
    let names: HashMap<_, _> = state
        .db
        .call(|store| store.list_orgs())
        .await?
        .into_iter()
        .map(|org| (org.id, org.name))
        .collect();
    let joined = joined_orgs(&state, &principal).await?;
    let visible = projects
        .into_iter()
        .filter(|(project, _)| {
            joined
                .as_ref()
                .is_none_or(|orgs| orgs.contains(&project.org_id))
        })
        .filter_map(|(project, environments)| {
            let org = names.get(&project.org_id)?.clone();
            Some(summary(project, org, environments))
        })
        .collect();
    Ok(Json(visible))
}

/// Create a project with a `production` environment, for the org's admins
#[utoipa::path(post, path = "/v1/projects", tag = "projects", request_body = CreateProject, responses((status = 201, description = "Project created", body = ProjectSummary), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Json(request): Json<CreateProject>,
) -> Result<(StatusCode, Json<ProjectSummary>)> {
    let org = owning_org(&state, &principal, request.org.as_ref()).await?;
    match role_in(&state, &principal, org.id).await? {
        None => return Err(Error::OrgNotFound(org.name)),
        Some(role) if !role.at_least(OrgRole::Admin) => return Err(forbidden(OrgRole::Admin)),
        Some(_) => {}
    }
    let first = environments::create_project(&state, org.id, &request.name).await?;
    let created = ProjectSummary {
        name: request.name,
        org: org.name,
        environments: vec![first.name],
    };
    Ok((StatusCode::CREATED, Json(created)))
}

/// Delete a project whose environments have no services or backups left, for the org's admins
#[utoipa::path(delete, path = "/v1/projects/{project}", tag = "projects", params(("project" = String, Path, description = "Project name")), responses((status = 204, description = "Project deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ProjectPath { project }): Path<ProjectPath>,
) -> Result<StatusCode> {
    let found = environments::project(&state, &project).await?;
    require_project(&state, &principal, &found, OrgRole::Admin).await?;
    environments::remove_project(&state, &project).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Create an environment, with its own private network, for the org's admins
#[utoipa::path(post, path = "/v1/projects/{project}/environments", tag = "projects", params(("project" = String, Path, description = "Project name")), request_body = CreateEnvironment, responses((status = 201, description = "Environment created", body = String), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn create_environment(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(ProjectPath { project }): Path<ProjectPath>,
    Json(request): Json<CreateEnvironment>,
) -> Result<(StatusCode, Json<Name>)> {
    let found = environments::project(&state, &project).await?;
    require_project(&state, &principal, &found, OrgRole::Admin).await?;
    let created = environments::create_environment(&state, &project, &request.name).await?;
    Ok((StatusCode::CREATED, Json(created.name)))
}

/// Delete an environment that has no services or backups left, for the org's admins
#[utoipa::path(delete, path = "/v1/projects/{project}/environments/{environment}", tag = "projects", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name")), responses((status = 204, description = "Environment deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 403, description = "Not allowed for this user", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove_environment(
    State(state): State<AppState>,
    Extension(principal): Extension<Principal>,
    Path(EnvironmentPath {
        project,
        environment,
    }): Path<EnvironmentPath>,
) -> Result<StatusCode> {
    let found = environments::project(&state, &project).await?;
    require_project(&state, &principal, &found, OrgRole::Admin).await?;
    environments::remove_environment(&state, &project, &environment).await?;
    Ok(StatusCode::NO_CONTENT)
}

// `None` for server admins, who see every org
async fn joined_orgs(
    state: &AppState,
    principal: &Principal,
) -> Result<Option<HashSet<bird_core::OrgId>>> {
    match principal {
        Principal::User(user) if user.role == UserRole::Member => {
            let user_id = user.id;
            let joined = state.db.call(move |store| store.user_orgs(user_id)).await?;
            Ok(Some(joined.into_iter().map(|(org, _)| org.id).collect()))
        }
        Principal::Root | Principal::User(_) => Ok(None),
    }
}

// the org named, else the only one the user administers, else `default` for server admins
async fn owning_org(state: &AppState, principal: &Principal, named: Option<&Name>) -> Result<Org> {
    if let Some(name) = named {
        return orgs::find(state, name).await;
    }
    let user = match principal {
        Principal::User(user) if user.role == UserRole::Member => user,
        Principal::Root | Principal::User(_) => {
            return orgs::find(state, &DEFAULT_ORG.parse()?).await;
        }
    };
    let user_id = user.id;
    let mut administered: Vec<Org> = state
        .db
        .call(move |store| store.user_orgs(user_id))
        .await?
        .into_iter()
        .filter(|(_, role)| role.at_least(OrgRole::Admin))
        .map(|(org, _)| org)
        .collect();
    match (administered.pop(), administered.is_empty()) {
        (Some(org), true) => Ok(org),
        (None, _) => Err(Error::PickOrg(
            "you administer no org, ask an org owner to make you an admin".to_owned(),
        )),
        (Some(_), false) => Err(Error::PickOrg(
            "you administer more than one org, name one with --org".to_owned(),
        )),
    }
}

fn summary(project: Project, org: Name, environments: Vec<Environment>) -> ProjectSummary {
    ProjectSummary {
        name: project.name,
        org,
        environments: environments.into_iter().map(|env| env.name).collect(),
    }
}
