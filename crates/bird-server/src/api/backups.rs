use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::{BackupInfo, BackupVolumeInfo, ErrorBody, RestoreRequest, RestoreResponse};
use bird_core::{Backup, BackupId, BackupSchedule, Name};

use crate::state::AppState;
use crate::{Result, backups};

/// Back up a service's volumes
#[utoipa::path(post, path = "/v1/services/{name}/backups", tag = "backups", params(("name" = String, Path, description = "Service name")), responses((status = 201, description = "Every volume copied while the machines were paused", body = BackupInfo), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service not found", body = ErrorBody), (status = 409, description = "The service has no volumes, or another operation is in progress", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<(StatusCode, Json<BackupInfo>)> {
    // detached so a disconnecting client cannot leave machines paused
    let backup = detach(async move { backups::create(&state, &name).await }).await?;
    Ok((StatusCode::CREATED, Json(info(backup))))
}

/// List backups of a service, newest first
#[utoipa::path(get, path = "/v1/services/{name}/backups", tag = "backups", params(("name" = String, Path, description = "Service name, also of a removed service")), responses((status = 200, description = "Backups, newest first", body = Vec<BackupInfo>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<Vec<BackupInfo>>> {
    let backups = backups::list(&state, &name).await?;
    Ok(Json(backups.into_iter().map(info).collect()))
}

/// Replace a service's volume data with a backup
#[utoipa::path(post, path = "/v1/services/{name}/backups/{id}/restore", tag = "backups", params(("name" = String, Path, description = "Service name"), ("id" = String, Path, description = "Backup id")), request_body = RestoreRequest, responses((status = 200, description = "Data restored and machines started again; the replaced data is kept as a new backup", body = RestoreResponse), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or backup not found", body = ErrorBody), (status = 409, description = "The backup does not fit the service, or another operation is in progress", body = ErrorBody)))]
pub(crate) async fn restore(
    State(state): State<AppState>,
    Path((name, id)): Path<(Name, BackupId)>,
    Json(request): Json<RestoreRequest>,
) -> Result<Json<RestoreResponse>> {
    let restored =
        detach(
            async move { backups::restore(&state, &name, id, request.allow_image_change).await },
        )
        .await?;
    Ok(Json(RestoreResponse {
        restored: restored.backup,
        safety_backup: restored.safety_backup,
    }))
}

/// Delete a backup and its archives
#[utoipa::path(delete, path = "/v1/services/{name}/backups/{id}", tag = "backups", params(("name" = String, Path, description = "Service name"), ("id" = String, Path, description = "Backup id")), responses((status = 204, description = "Backup deleted"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Backup not found", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    Path((name, id)): Path<(Name, BackupId)>,
) -> Result<StatusCode> {
    backups::remove(&state, &name, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Back up a service's volumes on a schedule
#[utoipa::path(put, path = "/v1/services/{name}/backups/schedule", tag = "backups", params(("name" = String, Path, description = "Service name")), request_body = BackupSchedule, responses((status = 200, description = "Schedule saved; the first backup runs within a minute when none was scheduled before", body = BackupSchedule), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service not found", body = ErrorBody), (status = 409, description = "The service has no volumes", body = ErrorBody)))]
pub(crate) async fn set_schedule(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(schedule): Json<BackupSchedule>,
) -> Result<Json<BackupSchedule>> {
    backups::set_schedule(&state, &name, schedule).await?;
    Ok(Json(schedule))
}

/// Stop backing up a service on a schedule, keeping the backups it made
#[utoipa::path(delete, path = "/v1/services/{name}/backups/schedule", tag = "backups", params(("name" = String, Path, description = "Service name")), responses((status = 204, description = "Schedule removed"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service not found", body = ErrorBody)))]
pub(crate) async fn clear_schedule(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<StatusCode> {
    backups::clear_schedule(&state, &name).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn detach<T: Send + 'static>(
    work: impl Future<Output = Result<T>> + Send + 'static,
) -> Result<T> {
    match tokio::spawn(work).await {
        Ok(result) => result,
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}

fn info(backup: Backup) -> BackupInfo {
    BackupInfo {
        id: backup.id,
        service: backup.service,
        trigger: backup.trigger,
        storage: backup.storage,
        volumes: backup
            .volumes
            .into_iter()
            .map(|volume| BackupVolumeInfo {
                name: volume.name,
                size_bytes: volume.size_bytes,
            })
            .collect(),
        created_at: backup.created_at,
    }
}
