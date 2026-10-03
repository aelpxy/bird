use std::time::Duration;

use bird_core::{Deployment, Machine, MachineState};

use super::machine::set_state;
use crate::routing;
use crate::state::AppState;

// databases get time to flush and shut down cleanly before their volume changes hands
const STOP_GRACE: Duration = Duration::from_secs(30);

// a volume can only be used by one machine, so the old one stops before the new one starts
pub(super) async fn stop_previous(state: &AppState, previous: Option<&Deployment>) -> Vec<Machine> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let deployment_id = previous.id;
    let machines = match state
        .db
        .call(move |store| store.list_machines(deployment_id))
        .await
    {
        Ok(machines) => machines,
        Err(err) => {
            tracing::warn!(error = %err, "could not list machines to stop");
            return Vec::new();
        }
    };
    let running: Vec<Machine> = machines
        .into_iter()
        .filter(|m| m.state == MachineState::Running)
        .collect();
    // stopped, not stopping: cleanup retires stopping machines, these must survive for a restore
    for machine in &running {
        set_state(state, machine.id, MachineState::Stopped).await;
    }
    if let Err(err) = routing::refresh(state).await {
        tracing::warn!(error = %err, "could not refresh routes before stopping");
    }
    for machine in &running {
        if let Some(container_id) = &machine.container_id
            && let Err(err) = state.podman.stop_container(container_id, STOP_GRACE).await
        {
            tracing::warn!(machine = %machine.id, error = %err, "could not stop machine");
        }
    }
    running
}

// a failed deploy should not leave the service down when the old machine still works
pub(super) async fn restore(state: &AppState, machines: &[Machine]) {
    for machine in machines {
        let Some(container_id) = &machine.container_id else {
            continue;
        };
        if let Err(err) = state.podman.start_container(container_id).await {
            tracing::warn!(machine = %machine.id, error = %err, "could not restart previous machine");
            continue;
        }
        tracing::info!(machine = %machine.id, "previous machine restored");
        set_state(state, machine.id, MachineState::Running).await;
    }
    if let Err(err) = routing::refresh(state).await {
        tracing::warn!(error = %err, "could not refresh routes after restoring");
    }
}
