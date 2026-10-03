use std::net::{Ipv4Addr, SocketAddr};

use bird_core::{Machine, MachineState};
use bird_podman::ContainerState;

use crate::Result;
use crate::deploy::set_state;
use crate::state::AppState;

pub(crate) async fn run(state: &AppState) {
    let machines = match state.db.call(|store| store.list_live_machines()).await {
        Ok(machines) => machines,
        Err(err) => {
            tracing::error!(error = %err, "could not load machines to reconcile");
            return;
        }
    };
    tracing::info!(
        machines = machines.len(),
        "reconciling machines with podman"
    );
    for machine in machines {
        if let Err(err) = reconcile_machine(state, &machine).await {
            tracing::warn!(machine = %machine.id, error = %err, "machine could not be recovered");
            set_state(state, machine.id, MachineState::Failed).await;
        }
    }
}

async fn reconcile_machine(state: &AppState, machine: &Machine) -> Result<()> {
    let Some(container_id) = machine.container_id.as_deref() else {
        tracing::warn!(machine = %machine.id, "machine has no container");
        set_state(state, machine.id, MachineState::Failed).await;
        return Ok(());
    };
    let deployment_id = machine.deployment_id;
    let Some(deployment) = state
        .db
        .call(move |store| store.deployment(deployment_id))
        .await?
    else {
        set_state(state, machine.id, MachineState::Failed).await;
        return Ok(());
    };

    let mut info = match state.podman.inspect_container(container_id).await {
        Ok(info) => info,
        Err(bird_podman::Error::NotFound { .. }) => {
            tracing::warn!(machine = %machine.id, "container is gone");
            set_state(state, machine.id, MachineState::Failed).await;
            return Ok(());
        }
        Err(err) => return Err(err.into()),
    };
    if info.state != ContainerState::Running {
        tracing::info!(machine = %machine.id, "starting stopped container");
        state.podman.start_container(container_id).await?;
        info = state.podman.inspect_container(container_id).await?;
    }

    let Some(host_port) = info.host_port(deployment.port) else {
        set_state(state, machine.id, MachineState::Failed).await;
        return Ok(());
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, host_port));
    let machine_id = machine.id;
    state
        .db
        .call(move |store| {
            store.set_machine_address(machine_id, address)?;
            store.set_machine_state(machine_id, MachineState::Running)
        })
        .await?;
    Ok(())
}
