use bird_core::{Machine, MachineState, Service};

use crate::deploy::{destroy_container, set_state};
use crate::routing;
use crate::state::AppState;

// newest machines go first; each leaves routing before its container stops
pub(super) async fn retire_extra(state: &AppState, service: &Service, extra: &[&Machine]) {
    for machine in extra.iter().rev() {
        set_state(state, machine.id, MachineState::Stopping).await;
        if let Err(err) = routing::refresh(state).await {
            tracing::warn!(error = %err, "could not refresh routes before scaling down");
        }
        if let Some(container_id) = &machine.container_id {
            destroy_container(state, container_id).await;
        }
        set_state(state, machine.id, MachineState::Destroyed).await;
        tracing::info!(service = %service.name, machine = %machine.id, "machine removed by scale down");
    }
}
