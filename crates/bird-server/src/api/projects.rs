use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::{CreateEnvironment, CreateProject, ErrorBody, ProjectSummary};
use bird_core::{Environment, Name, Project};
use serde::Deserialize;

use crate::state::AppState;
use crate::{Result, environments};

#[derive(Deserialize)]
pub(crate) struct ProjectPath {
    project: Name,
}

#[derive(Deserialize)]
pub(crate) struct EnvironmentPath {
    project: Name,
    environment: Name,
}

/// List projects and their environments
#[utoipa::path(get, path = "/v1/projects", tag = "projects", responses((status = 200, description = "Projects with their environments", body = Vec<ProjectSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn list(State(state): State<AppState>) -> Result<Json<Vec<ProjectSummary>>> {
    let projects = environments::list(&state).await?;
    Ok(Json(projects.into_iter().map(summary).collect()))
}

/// Create a project with a `production` environment
#[utoipa::path(post, path = "/v1/projects", tag = "projects", request_body = CreateProject, responses((status = 201, description = "Project created", body = ProjectSummary), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Json(request): Json<CreateProject>,
) -> Result<(StatusCode, Json<ProjectSummary>)> {
    let first = environments::create_project(&state, &request.name).await?;
    let created = ProjectSummary {
        name: request.name,
        environments: vec![first.name],
    };
    Ok((StatusCode::CREATED, Json(created)))
}

/// Delete a project whose environments have no services or backups left
#[utoipa::path(delete, path = "/v1/projects/{project}", tag = "projects", params(("project" = String, Path, description = "Project name")), responses((status = 204, description = "Project deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    Path(ProjectPath { project }): Path<ProjectPath>,
) -> Result<StatusCode> {
    environments::remove_project(&state, &project).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Create an environment, with its own private network
#[utoipa::path(post, path = "/v1/projects/{project}/environments", tag = "projects", params(("project" = String, Path, description = "Project name")), request_body = CreateEnvironment, responses((status = 201, description = "Environment created", body = String), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn create_environment(
    State(state): State<AppState>,
    Path(ProjectPath { project }): Path<ProjectPath>,
    Json(request): Json<CreateEnvironment>,
) -> Result<(StatusCode, Json<Name>)> {
    let created = environments::create_environment(&state, &project, &request.name).await?;
    Ok((StatusCode::CREATED, Json(created.name)))
}

/// Delete an environment that has no services or backups left
#[utoipa::path(delete, path = "/v1/projects/{project}/environments/{environment}", tag = "projects", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name")), responses((status = 204, description = "Environment deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove_environment(
    State(state): State<AppState>,
    Path(EnvironmentPath {
        project,
        environment,
    }): Path<EnvironmentPath>,
) -> Result<StatusCode> {
    environments::remove_environment(&state, &project, &environment).await?;
    Ok(StatusCode::NO_CONTENT)
}

fn summary((project, environments): (Project, Vec<Environment>)) -> ProjectSummary {
    ProjectSummary {
        name: project.name,
        environments: environments.into_iter().map(|env| env.name).collect(),
    }
}
