mod create;
mod database;
mod restore;
mod schedule;
pub(crate) mod storage;

use bird_core::{Backup, BackupId, EnvironmentId, Name};

pub(crate) use create::create;
pub(crate) use database::DatabaseBackups;
pub(crate) use restore::restore;
pub(crate) use schedule::Scheduler;
pub(crate) use schedule::{clear as clear_schedule, set as set_schedule};
pub(crate) use storage::{BackupStorage, LocalDir, S3, S3Settings};

use crate::state::AppState;
use crate::{Error, Result};
use storage::Adapter;

pub(crate) async fn list(
    state: &AppState,
    environment_id: EnvironmentId,
    service: &Name,
) -> Result<Vec<Backup>> {
    let service = service.clone();
    state
        .db
        .call(move |store| store.list_backups(environment_id, &service))
        .await
}

pub(crate) async fn remove(
    state: &AppState,
    environment: EnvironmentId,
    service: &Name,
    id: BackupId,
) -> Result<()> {
    let backup = find(state, environment, service, id).await?;
    ensure_reachable(state, &backup)?;
    for volume in &backup.volumes {
        state.backups.delete(&volume.key).await?;
    }
    state.db.call(move |store| store.delete_backup(id)).await?;
    tracing::info!(service = %service, backup = %id, "backup deleted");
    Ok(())
}

async fn find(
    state: &AppState,
    environment_id: EnvironmentId,
    service: &Name,
    id: BackupId,
) -> Result<Backup> {
    state
        .db
        .call(move |store| store.backup(id))
        .await?
        .filter(|backup| backup.environment_id == environment_id && backup.service == *service)
        .ok_or(Error::BackupNotFound(id))
}

fn ensure_reachable(state: &AppState, backup: &Backup) -> Result<()> {
    if backup.storage == state.backups.name() {
        Ok(())
    } else {
        Err(Error::BackupStorageMismatch(backup.storage.clone()))
    }
}
