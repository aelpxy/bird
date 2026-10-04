use std::time::Duration;

use bird_core::{Deployment, Machine, MachineState, Name, Service, ServiceState};

use super::machine::boot;
use super::{OPERATION_PATIENCE, set_state};
use crate::state::AppState;
use crate::{Error, Result, routing};

// databases get time to shut down cleanly, the same as when a volume deploy stops them
const STOP_GRACE: Duration = Duration::from_secs(30);

// containers are kept, so logs survive and starting again needs no pull or new container
pub(crate) async fn stop(state: &AppState, name: &Name) -> Result<()> {
    let _ticket = state.deploys.wait_for(name, OPERATION_PATIENCE).await?;
    let service = state.service(name).await?;
    let service_id = service.id;
    let machines = state
        .db
        .call(move |store| {
            store.set_service_state(service_id, ServiceState::Stopped)?;
            let Some(deployment) = store.active_deployment(service_id)? else {
                return Ok(Vec::new());
            };
            store.list_machines(deployment.id)
        })
        .await?;
    let live: Vec<Machine> = machines
        .into_iter()
        .filter(|m| {
            matches!(
                m.state,
                MachineState::Created | MachineState::Starting | MachineState::Running
            )
        })
        .collect();
    // recorded first so the proxy stops sending traffic before the app goes away
    for machine in &live {
        set_state(state, machine.id, MachineState::Stopped).await;
    }
    routing::refresh(state).await?;
    for machine in &live {
        let Some(container_id) = &machine.container_id else {
            continue;
        };
        if let Err(err) = state.podman.stop_container(container_id, STOP_GRACE).await {
            tracing::warn!(machine = %machine.id, error = %err, "could not stop container");
        }
    }
    tracing::info!(service = %name, machines = live.len(), "service stopped");
    Ok(())
}

// machines the supervisor would add, like extra replicas, follow once the service runs again
pub(crate) async fn start(state: &AppState, name: &Name) -> Result<()> {
    let _ticket = state.deploys.wait_for(name, OPERATION_PATIENCE).await?;
    let service = state.service(name).await?;
    let service_id = service.id;
    let found = state
        .db
        .call(move |store| {
            store.set_service_state(service_id, ServiceState::Running)?;
            let Some(deployment) = store.active_deployment(service_id)? else {
                return Ok(None);
            };
            let machines = store.list_machines(deployment.id)?;
            Ok(Some((deployment, machines)))
        })
        .await?;
    let Some((deployment, machines)) = found else {
        return Err(Error::NeverDeployed(name.clone()));
    };
    let service = Service {
        state: ServiceState::Running,
        ..service
    };
    let mut outcome = Ok(());
    for machine in machines.iter().filter(|m| m.state == MachineState::Stopped) {
        if let Err(err) = bring_up(state, &service, &deployment, machine).await
            && outcome.is_ok()
        {
            outcome = Err(err);
        }
    }
    routing::refresh(state).await?;
    state.reconcile_now.notify_one();
    tracing::info!(service = %name, "service started");
    outcome
}

// one machine at a time, so a service with replicas keeps serving throughout
pub(crate) async fn restart(state: &AppState, name: &Name) -> Result<()> {
    let _ticket = state.deploys.wait_for(name, OPERATION_PATIENCE).await?;
    let service = state.service(name).await?;
    if service.state == ServiceState::Stopped {
        return Err(Error::ServiceStopped(name.clone()));
    }
    let service_id = service.id;
    let found = state
        .db
        .call(move |store| {
            let Some(deployment) = store.active_deployment(service_id)? else {
                return Ok(None);
            };
            let machines = store.list_machines(deployment.id)?;
            Ok(Some((deployment, machines)))
        })
        .await?;
    let Some((deployment, machines)) = found else {
        return Err(Error::NoMachines(name.clone()));
    };
    let running: Vec<&Machine> = machines
        .iter()
        .filter(|m| m.state == MachineState::Running)
        .collect();
    if running.is_empty() {
        return Err(Error::NoMachines(name.clone()));
    }
    for machine in running {
        set_state(state, machine.id, MachineState::Starting).await;
        routing::refresh(state).await?;
        if let Some(container_id) = &machine.container_id
            && let Err(err) = state.podman.stop_container(container_id, STOP_GRACE).await
        {
            set_state(state, machine.id, MachineState::Failed).await;
            return Err(err.into());
        }
        bring_up(state, &service, &deployment, machine).await?;
        routing::refresh(state).await?;
        tracing::info!(service = %name, machine = %machine.id, "machine restarted");
    }
    Ok(())
}

// a machine that does not come back is marked failed, and the supervisor replaces it
async fn bring_up(
    state: &AppState,
    service: &Service,
    deployment: &Deployment,
    machine: &Machine,
) -> Result<()> {
    let Some(container_id) = &machine.container_id else {
        set_state(state, machine.id, MachineState::Failed).await;
        return Ok(());
    };
    set_state(state, machine.id, MachineState::Starting).await;
    match boot(state, container_id, deployment, service).await {
        Ok(address) => {
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
        Err(err) => {
            set_state(state, machine.id, MachineState::Failed).await;
            Err(err)
        }
    }
}
