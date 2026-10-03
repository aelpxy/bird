use bird_core::Name;

use crate::state::AppState;
use crate::{Error, Result, routing};

use super::OPERATION_PATIENCE;
use super::machine::destroy_container;

pub(crate) async fn remove_service(state: &AppState, name: Name, purge: bool) -> Result<()> {
    let _ticket = state.deploys.wait_for(&name, OPERATION_PATIENCE).await?;
    let environment_id = state.environment_id;
    let lookup = name.clone();
    let service = state
        .db
        .call(move |store| store.service_by_name(environment_id, &lookup))
        .await?
        .ok_or_else(|| Error::ServiceNotFound(name.clone()))?;

    let service_id = service.id;
    let volumes = state
        .db
        .call(move |store| store.list_volumes(service_id))
        .await?;
    if !volumes.is_empty() && !purge {
        return Err(Error::HasVolumes(name));
    }
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
    tracing::info!(service = %name, "service removed");
    Ok(())
}
