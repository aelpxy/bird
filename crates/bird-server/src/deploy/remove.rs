use bird_core::{EnvironmentId, Name};

use crate::state::AppState;
use crate::{Error, Result, images, routing};

use super::OPERATION_PATIENCE;
use super::machine::destroy_container;
use super::references::dependents;

pub(crate) async fn remove_service(
    state: &AppState,
    environment: EnvironmentId,
    name: Name,
    purge: bool,
) -> Result<()> {
    let _ticket = state
        .deploys
        .wait_for(environment, &name, OPERATION_PATIENCE)
        .await?;
    let service = state.service(environment, &name).await?;

    let dependents = dependents(state, environment, &name).await?;
    if !dependents.is_empty() {
        return Err(Error::HasDependents(name, dependents));
    }

    let service_id = service.id;
    let volumes = state
        .db
        .call(move |store| store.list_volumes(service_id))
        .await?;
    if !volumes.is_empty() && !purge {
        return Err(Error::HasVolumes(name));
    }
    let images: Vec<_> = state
        .db
        .call(move |store| store.list_deployments(service_id))
        .await?
        .into_iter()
        .map(|d| d.image)
        .collect();
    let machines = state
        .db
        .call(move |store| store.list_service_machines(service_id))
        .await?;
    for container_id in machines.iter().filter_map(|m| m.container_id.as_deref()) {
        destroy_container(state, container_id).await;
    }
    for volume in &volumes {
        match state.podman.remove_volume(&volume.podman_name()).await {
            Ok(()) | Err(bird_podman::Error::NotFound { .. }) => {
                tracing::info!(service = %name, volume = %volume.name, "volume deleted");
            }
            Err(err) => return Err(err.into()),
        }
    }

    state
        .db
        .call(move |store| store.delete_service(service_id))
        .await?;
    routing::refresh(state).await?;
    state.domains_changed.notify_one();
    images::remove_orphaned(state, images).await;
    tracing::info!(service = %name, "service removed");
    Ok(())
}
