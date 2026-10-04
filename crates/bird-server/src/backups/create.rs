use std::time::Duration;

use bird_core::{
    Backup, BackupId, BackupTrigger, BackupVolume, MachineState, Name, Service, Volume,
};

use super::storage::Adapter;
use crate::deploy::OPERATION_PATIENCE;
use crate::state::AppState;
use crate::{Error, Result};

const COPY_TIMEOUT: Duration = Duration::from_hours(1);

pub(crate) async fn create(state: &AppState, name: &Name) -> Result<Backup> {
    let _ticket = state.deploys.wait_for(name, OPERATION_PATIENCE).await?;
    let service = state.service(name).await?;
    snapshot(state, &service, BackupTrigger::Manual).await
}

// the caller holds the service's deploy ticket, so the supervisor leaves the paused machines alone
pub(super) async fn snapshot(
    state: &AppState,
    service: &Service,
    trigger: BackupTrigger,
) -> Result<Backup> {
    let service_id = service.id;
    let (volumes, machines) = state
        .db
        .call(move |store| {
            Ok((
                store.list_volumes(service_id)?,
                store.list_service_machines(service_id)?,
            ))
        })
        .await?;
    if volumes.is_empty() {
        return Err(Error::NothingToBackUp(service.name.clone()));
    }
    let running: Vec<String> = machines
        .into_iter()
        .filter(|machine| machine.state == MachineState::Running)
        .filter_map(|machine| machine.container_id)
        .collect();

    let id = BackupId::generate();
    tracing::info!(service = %service.name, backup = %id, "backing up");
    let mut paused = Vec::new();
    let mut copied = Vec::new();
    // paused rather than stopped: the copy is what a crash at that instant would leave, which
    // databases recover from, and the app keeps its connections instead of restarting
    let outcome = match pause(state, &running, &mut paused).await {
        Ok(()) => tokio::time::timeout(COPY_TIMEOUT, copy(state, id, &volumes, &mut copied))
            .await
            .unwrap_or_else(|_| {
                Err(Error::BackupFailed(format!(
                    "copying took longer than {} minutes",
                    COPY_TIMEOUT.as_secs() / 60
                )))
            }),
        Err(err) => Err(err),
    };
    resume(state, &paused).await;

    let backup = Backup {
        id,
        environment_id: service.environment_id,
        service: service.name.clone(),
        trigger,
        storage: state.backups.name().to_owned(),
        volumes: copied,
        created_at: 0,
    };
    let saved = match outcome {
        Ok(()) => {
            let record = backup.clone();
            state
                .db
                .call(move |store| store.create_backup(record))
                .await
        }
        Err(err) => Err(err),
    };
    let backup = match saved {
        Ok(saved) => saved,
        Err(err) => {
            discard(state, &backup.volumes).await;
            tracing::warn!(service = %service.name, backup = %id, error = %err, "backup failed");
            return Err(err);
        }
    };
    let bytes: u64 = backup.volumes.iter().map(|v| v.size_bytes).sum();
    tracing::info!(service = %service.name, backup = %id, bytes, "backed up");
    Ok(backup)
}

async fn pause(state: &AppState, running: &[String], paused: &mut Vec<String>) -> Result<()> {
    for container_id in running {
        state.podman.pause_container(container_id).await?;
        paused.push(container_id.clone());
    }
    Ok(())
}

async fn resume(state: &AppState, paused: &[String]) {
    for container_id in paused {
        if let Err(err) = state.podman.unpause_container(container_id).await {
            tracing::error!(container = container_id, error = %err, "could not resume machine after backup, the supervisor will retry");
        }
    }
}

async fn copy(
    state: &AppState,
    id: BackupId,
    volumes: &[Volume],
    copied: &mut Vec<BackupVolume>,
) -> Result<()> {
    for volume in volumes {
        let key = format!("{id}/{}.tar", volume.name);
        let archive = state.podman.export_volume(&volume.podman_name()).await?;
        let size_bytes = state.backups.put(&key, archive).await?;
        copied.push(BackupVolume {
            name: volume.name.clone(),
            lineage: volume.lineage.clone(),
            key,
            size_bytes,
        });
    }
    Ok(())
}

async fn discard(state: &AppState, copied: &[BackupVolume]) {
    for volume in copied {
        if let Err(err) = state.backups.delete(&volume.key).await {
            tracing::warn!(key = %volume.key, error = %err, "could not delete part of a failed backup");
        }
    }
}
