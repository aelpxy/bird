use bird_core::Name;

use crate::state::AppState;
use crate::{Error, Result, routing};

use super::machine::destroy_container;

pub(crate) async fn remove_service(state: &AppState, name: Name) -> Result<()> {
    let _ticket = state.deploys.begin(&name)?;
    let environment_id = state.environment_id;
    let lookup = name.clone();
    let service = state
        .db
        .call(move |store| store.service_by_name(environment_id, &lookup))
        .await?
        .ok_or_else(|| Error::ServiceNotFound(name.clone()))?;

    let service_id = service.id;
    let machines = state
        .db
        .call(move |store| store.list_service_machines(service_id))
        .await?;
    for container_id in machines.iter().filter_map(|m| m.container_id.as_deref()) {
        destroy_container(state, container_id).await;
    }

    state
        .db
        .call(move |store| store.delete_service(service_id))
        .await?;
    routing::refresh(state).await?;
    tracing::info!(service = %name, "service removed");
    Ok(())
}
