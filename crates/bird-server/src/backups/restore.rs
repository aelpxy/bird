use bird_core::{
    Backup, BackupId, BackupTrigger, BackupVolume, Deployment, EnvironmentId, ImageLineage,
    ImageRef, Machine, MachineState, Name, Service, ServiceState, Volume,
};

use super::create::snapshot;
use super::storage::Adapter;
use super::{ensure_reachable, find};
use crate::deploy::{OPERATION_PATIENCE, destroy_container, launch, set_state};
use crate::state::AppState;
use crate::{Error, Result, labels, routing};

pub(crate) struct Restored {
    pub(crate) backup: BackupId,
    pub(crate) safety_backup: BackupId,
}

pub(crate) async fn restore(
    state: &AppState,
    environment: EnvironmentId,
    name: &Name,
    id: BackupId,
    allow_image_change: bool,
) -> Result<Restored> {
    let _ticket = state
        .deploys
        .wait_for(environment, name, OPERATION_PATIENCE)
        .await?;
    let service = state.service(environment, name).await?;
    let backup = find(state, environment, name, id).await?;
    ensure_reachable(state, &backup)?;
    let service_id = service.id;
    let (volumes, machines, active) = state
        .db
        .call(move |store| {
            Ok((
                store.list_volumes(service_id)?,
                store.list_service_machines(service_id)?,
                store.active_deployment(service_id)?,
            ))
        })
        .await?;
    let targets = pair(&backup, &volumes, &service.name)?;
    if !allow_image_change {
        let image = active.as_ref().map_or(&service.image, |d| &d.image);
        check_lineage(&targets, image)?;
    }
    for (saved, _) in &targets {
        if !state.backups.exists(&saved.key).await? {
            return Err(Error::BackupDataMissing(saved.key.clone()));
        }
    }

    let restored = match download(state, &targets).await {
        Ok(()) => swap(state, &service, id, &machines, &targets, active).await,
        Err(err) => Err(err),
    };
    unstage(state, &targets).await;
    restored
}

async fn swap(
    state: &AppState,
    service: &Service,
    id: BackupId,
    machines: &[Machine],
    targets: &[(&BackupVolume, &Volume)],
    active: Option<Deployment>,
) -> Result<Restored> {
    let name = &service.name;
    // nothing is touched until the current data is saved, so a bad restore can be undone
    let safety = snapshot(state, service, BackupTrigger::Restore).await?;
    tracing::info!(service = %name, backup = %id, safety_backup = %safety.id, "restoring");
    let restored = async {
        replace_data(state, machines, targets).await?;
        relaunch(state, service, active).await
    }
    .await;
    if let Err(err) = restored {
        tracing::error!(service = %name, backup = %id, safety_backup = %safety.id, error = %err, "restore failed");
        return Err(Error::RestoreFailed {
            safety: safety.id,
            reason: err.to_string(),
        });
    }
    tracing::info!(service = %name, backup = %id, "restored");
    Ok(Restored {
        backup: id,
        safety_backup: safety.id,
    })
}

fn pair<'a>(
    backup: &'a Backup,
    volumes: &'a [Volume],
    service: &Name,
) -> Result<Vec<(&'a BackupVolume, &'a Volume)>> {
    backup
        .volumes
        .iter()
        .map(|saved| {
            volumes
                .iter()
                .find(|volume| volume.name == saved.name)
                .map(|volume| (saved, volume))
                .ok_or_else(|| Error::BackupVolumeMissing {
                    service: service.clone(),
                    volume: saved.name.clone(),
                })
        })
        .collect()
}

// the next machine runs the current image on the restored data, so it must be the same kind
fn check_lineage(targets: &[(&BackupVolume, &Volume)], image: &ImageRef) -> Result<()> {
    let now = ImageLineage::of(image);
    match targets.iter().find_map(|(saved, _)| {
        saved
            .lineage
            .as_ref()
            .filter(|was| !now.matches(was))
            .map(|was| (saved, was))
    }) {
        Some((saved, was)) => Err(Error::ImageChange {
            volume: saved.name.clone(),
            was: was.clone(),
            now: now.to_string(),
        }),
        None => Ok(()),
    }
}

// podman only adds files on import, so each volume is recreated empty first, which needs every
// machine using it gone
async fn replace_data(
    state: &AppState,
    machines: &[Machine],
    targets: &[(&BackupVolume, &Volume)],
) -> Result<()> {
    for machine in machines
        .iter()
        .filter(|m| m.state != MachineState::Destroyed)
    {
        if let Some(container_id) = &machine.container_id {
            destroy_container(state, container_id).await;
        }
        set_state(state, machine.id, MachineState::Destroyed).await;
    }
    routing::refresh(state).await?;
    for (saved, volume) in targets {
        let podman_name = volume.podman_name();
        match state.podman.remove_volume(&podman_name).await {
            Ok(()) | Err(bird_podman::Error::NotFound { .. }) => {}
            Err(err) => return Err(err.into()),
        }
        state
            .podman
            .ensure_volume(&podman_name, &labels::for_volume(volume))
            .await?;
        let archive = match state.backups.staging() {
            Some(staging) => staging.get(&saved.key).await?,
            None => state.backups.get(&saved.key).await?,
        };
        state.podman.import_volume(&podman_name, archive).await?;
        if let Some(lineage) = saved.lineage.clone() {
            let volume_id = volume.id;
            state
                .db
                .call(move |store| store.set_volume_lineage(volume_id, &lineage))
                .await?;
        }
        tracing::info!(volume = %volume.name, bytes = saved.size_bytes, "volume restored");
    }
    Ok(())
}

// fetched before the machines go, so the outage does not wait on a slow download
async fn download(state: &AppState, targets: &[(&BackupVolume, &Volume)]) -> Result<()> {
    let Some(staging) = state.backups.staging() else {
        return Ok(());
    };
    for (saved, _) in targets {
        let archive = state.backups.get(&saved.key).await?;
        staging.put(&saved.key, archive).await?;
    }
    Ok(())
}

async fn unstage(state: &AppState, targets: &[(&BackupVolume, &Volume)]) {
    let Some(staging) = state.backups.staging() else {
        return;
    };
    for (saved, _) in targets {
        if let Err(err) = staging.delete(&saved.key).await {
            tracing::warn!(key = %saved.key, error = %err, "could not delete a staged backup archive");
        }
    }
}

// a stopped service gets its data back and stays stopped until `bird start`
async fn relaunch(state: &AppState, service: &Service, active: Option<Deployment>) -> Result<()> {
    let Some(deployment) = active.filter(|_| service.state == ServiceState::Running) else {
        return Ok(());
    };
    let deployment_id = deployment.id;
    let env = state
        .db
        .call(move |store| store.deployment_variables(deployment_id))
        .await?;
    for _ in 0..service.replicas.get() {
        launch(state, service, &deployment, env.clone()).await?;
    }
    routing::refresh(state).await
}

#[cfg(test)]
mod tests {
    use bird_core::{BackupId, EnvironmentId, ServiceId, VolumeId};

    use super::*;

    fn backup(volumes: &[(&str, Option<&str>)]) -> Backup {
        Backup {
            id: BackupId::generate(),
            environment_id: EnvironmentId::generate(),
            service: "db".parse().unwrap(),
            trigger: BackupTrigger::Manual,
            storage: "local".to_owned(),
            volumes: volumes
                .iter()
                .map(|(name, lineage)| BackupVolume {
                    name: name.parse().unwrap(),
                    lineage: lineage.map(str::to_owned),
                    key: format!("x/{name}.tar"),
                    size_bytes: 1,
                })
                .collect(),
            created_at: 0,
        }
    }

    fn volume(name: &str) -> Volume {
        Volume {
            id: VolumeId::generate(),
            service_id: ServiceId::generate(),
            name: name.parse().unwrap(),
            mount_path: "/data".parse().unwrap(),
            lineage: None,
            created_at: 0,
        }
    }

    #[test]
    fn pairs_saved_volumes_with_current_ones_by_name() {
        let saved = backup(&[("data", None)]);
        let current = [volume("logs"), volume("data")];
        let service = "db".parse().unwrap();
        let pairs = pair(&saved, &current, &service).unwrap();
        assert_eq!(pairs.len(), 1);
        assert_eq!(pairs[0].1.name.as_str(), "data");
        assert!(matches!(
            pair(&saved, &[volume("logs")], &service),
            Err(Error::BackupVolumeMissing { .. })
        ));
    }

    #[test]
    fn refuses_data_from_another_kind_of_image() {
        let saved = backup(&[("data", Some("docker.io/library/postgres:17"))]);
        let current = [volume("data")];
        let service = "db".parse().unwrap();
        let pairs = pair(&saved, &current, &service).unwrap();
        let image = |raw: &str| raw.parse::<ImageRef>().unwrap();
        assert!(check_lineage(&pairs, &image("postgres:17.6")).is_ok());
        assert!(matches!(
            check_lineage(&pairs, &image("postgres:18")),
            Err(Error::ImageChange { .. })
        ));
        let unknown = backup(&[("data", None)]);
        let pairs = pair(&unknown, &current, &service).unwrap();
        assert!(check_lineage(&pairs, &image("anything:1")).is_ok());
    }
}
