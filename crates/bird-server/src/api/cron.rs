use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::{CronJobSummary, CronRunDetail, CronRunSummary, ErrorBody};
use bird_core::{CronRunId, CronTrigger, Name};
use serde::Deserialize;

use super::scope::ServiceScope;
use crate::state::AppState;
use crate::{Error, Result, cron};

// what `bird cron runs` shows; older runs are pruned past a few more than this
const LISTED_RUNS: u32 = 50;

/// List a service's cron jobs with their next and last run
#[utoipa::path(get, path = "/v1/projects/{project}/environments/{environment}/services/{name}/cron", tag = "cron", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name")), responses((status = 200, description = "Jobs by name; times are unix seconds and schedules are UTC", body = Vec<CronJobSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service not found", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
) -> Result<Json<Vec<CronJobSummary>>> {
    let service = state.service(environment, &name).await?;
    Ok(Json(cron::summaries(&state, service.id).await?))
}

/// List a cron job's runs, newest first
#[utoipa::path(get, path = "/v1/projects/{project}/environments/{environment}/services/{name}/cron/{job}/runs", tag = "cron", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ("job" = String, Path, description = "Job name")), responses((status = 200, description = "The latest runs, newest first", body = Vec<CronRunSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or job not found", body = ErrorBody)))]
pub(crate) async fn runs(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Path(JobPath { job }): Path<JobPath>,
) -> Result<Json<Vec<CronRunSummary>>> {
    let service = state.service(environment, &name).await?;
    let job = cron::find_job(&state, service.id, &name, &job).await?;
    let runs = state
        .db
        .call(move |store| store.list_cron_runs(job.id, LISTED_RUNS))
        .await?;
    Ok(Json(runs.iter().map(cron::summary).collect()))
}

/// Run a cron job now
#[utoipa::path(post, path = "/v1/projects/{project}/environments/{environment}/services/{name}/cron/{job}/runs", tag = "cron", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ("job" = String, Path, description = "Job name")), responses((status = 202, description = "The run started; poll it for its status and output", body = CronRunSummary), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or job not found, or the service was never deployed", body = ErrorBody), (status = 409, description = "The job is already running", body = ErrorBody), (status = 503, description = "Too many cron jobs are running", body = ErrorBody)))]
pub(crate) async fn trigger(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Path(JobPath { job }): Path<JobPath>,
) -> Result<(StatusCode, Json<CronRunSummary>)> {
    let service = state.service(environment, &name).await?;
    let job = cron::find_job(&state, service.id, &name, &job).await?;
    let run = cron::start(&state, &service, &job, CronTrigger::Manual).await?;
    Ok((StatusCode::ACCEPTED, Json(cron::summary(&run))))
}

/// Show a cron run with the end of its output
#[utoipa::path(get, path = "/v1/projects/{project}/environments/{environment}/services/{name}/cron/{job}/runs/{id}", tag = "cron", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ("job" = String, Path, description = "Job name"), ("id" = String, Path, description = "Run id")), responses((status = 200, description = "The run; output is the last 64 KiB, or why a skipped run did not start", body = CronRunDetail), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service, job or run not found", body = ErrorBody)))]
pub(crate) async fn run(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Path(RunPath { job, id }): Path<RunPath>,
) -> Result<Json<CronRunDetail>> {
    let service = state.service(environment, &name).await?;
    let job = cron::find_job(&state, service.id, &name, &job).await?;
    let run = state
        .db
        .call(move |store| store.cron_run(job.id, id))
        .await?
        .ok_or(Error::CronRunNotFound(id))?;
    Ok(Json(CronRunDetail {
        run: cron::summary(&run),
        output: run.output,
    }))
}

#[derive(Deserialize)]
pub(crate) struct JobPath {
    job: Name,
}

#[derive(Deserialize)]
pub(crate) struct RunPath {
    job: Name,
    id: CronRunId,
}
