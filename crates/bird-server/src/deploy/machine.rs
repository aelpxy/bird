use std::collections::BTreeMap;
use std::net::{Ipv4Addr, SocketAddr};
use std::time::Duration;

use bird_core::{
    Deployment, DeploymentId, EnvKey, HealthCheck, MachineId, MachineState, Port, Service,
};
use bird_podman::{ContainerSpec, ContainerState};
use tokio::time::Instant;

use crate::state::AppState;
use crate::{Error, Result, health, labels};

const READY_TIMEOUT: Duration = Duration::from_secs(60);
const READY_POLL: Duration = Duration::from_millis(250);
const STOP_GRACE: Duration = Duration::from_secs(10);
const FAILURE_LOG_LINES: u32 = 30;

pub(crate) async fn launch(
    state: &AppState,
    service: &Service,
    deployment: &Deployment,
    env: BTreeMap<EnvKey, String>,
) -> Result<()> {
    state.podman.pull_image(&deployment.image).await?;

    let service_id = service.id;
    let attached = state
        .db
        .call(move |store| store.list_volumes(service_id))
        .await?;
    let mounts = super::volumes::mounts(state, &attached).await?;

    let deployment_id = deployment.id;
    let machine = state
        .db
        .call(move |store| store.create_machine(deployment_id))
        .await?;
    let spec = ContainerSpec {
        name: format!("bird-{}-{}", service.name, machine.id),
        image: deployment.image.clone(),
        command: deployment.command.clone().map(Into::into),
        port: deployment.port,
        network: state.network.to_string(),
        aliases: labels::aliases(service),
        mounts,
        env,
        labels: labels::for_machine(service, deployment.id, machine.id),
    };

    let container_id = match state.podman.create_container(&spec).await {
        Ok(id) => id,
        Err(err) => {
            set_state(state, machine.id, MachineState::Failed).await;
            return Err(err.into());
        }
    };
    let recorded = container_id.clone();
    state
        .db
        .call(move |store| {
            store.set_machine_container(machine.id, &recorded)?;
            store.set_machine_state(machine.id, MachineState::Starting)
        })
        .await?;

    match boot(state, &container_id, deployment.port, service.health).await {
        Ok(address) => {
            state
                .db
                .call(move |store| {
                    store.set_machine_address(machine.id, address)?;
                    store.set_machine_state(machine.id, MachineState::Running)
                })
                .await?;
            tracing::info!(machine = %machine.id, %address, "machine running");
            Ok(())
        }
        Err(err) => {
            destroy_container(state, &container_id).await;
            set_state(state, machine.id, MachineState::Failed).await;
            Err(err)
        }
    }
}

async fn boot(
    state: &AppState,
    container_id: &str,
    port: Port,
    check: HealthCheck,
) -> Result<SocketAddr> {
    state.podman.start_container(container_id).await?;
    let info = state.podman.inspect_container(container_id).await?;
    let Some(host_port) = info.host_port(port) else {
        return Err(unhealthy(
            state,
            container_id,
            format!("port {port} was not published"),
        )
        .await);
    };
    let address = SocketAddr::from((Ipv4Addr::LOCALHOST, host_port));

    let deadline = Instant::now() + READY_TIMEOUT;
    loop {
        if health::probe(check, address).await {
            return Ok(address);
        }
        let info = state.podman.inspect_container(container_id).await?;
        if info.state != ContainerState::Running {
            let reason = format!("app exited before it started accepting {check} connections");
            return Err(unhealthy(state, container_id, reason).await);
        }
        if Instant::now() >= deadline {
            let reason = format!(
                "app did not accept {check} connections on port {port} within {}s",
                READY_TIMEOUT.as_secs()
            );
            return Err(unhealthy(state, container_id, reason).await);
        }
        tokio::time::sleep(READY_POLL).await;
    }
}

async fn unhealthy(state: &AppState, container_id: &str, reason: String) -> Error {
    let logs = match state.podman.logs(container_id, FAILURE_LOG_LINES).await {
        Ok(lines) => lines.into_iter().map(|line| line.text).collect(),
        Err(err) => {
            tracing::warn!(error = %err, "could not read logs of failed machine");
            Vec::new()
        }
    };
    Error::Unhealthy { reason, logs }
}

pub(crate) async fn retire(state: &AppState, deployment_id: DeploymentId) {
    let machines = match state
        .db
        .call(move |store| store.list_machines(deployment_id))
        .await
    {
        Ok(machines) => machines,
        Err(err) => {
            tracing::warn!(deployment = %deployment_id, error = %err, "could not list machines to retire");
            return;
        }
    };
    for machine in machines {
        if machine.state == MachineState::Destroyed {
            continue;
        }
        if let Some(container_id) = &machine.container_id {
            destroy_container(state, container_id).await;
        }
        set_state(state, machine.id, MachineState::Destroyed).await;
    }
}

pub(crate) async fn destroy_container(state: &AppState, container_id: &str) {
    match state.podman.stop_container(container_id, STOP_GRACE).await {
        Ok(()) | Err(bird_podman::Error::NotFound { .. }) => {}
        Err(err) => tracing::warn!(container = container_id, error = %err, "stop failed"),
    }
    match state.podman.remove_container(container_id).await {
        Ok(()) | Err(bird_podman::Error::NotFound { .. }) => {}
        Err(err) => tracing::warn!(container = container_id, error = %err, "remove failed"),
    }
}

pub(crate) async fn set_state(
    state: &AppState,
    machine_id: MachineId,
    machine_state: MachineState,
) {
    let result = state
        .db
        .call(move |store| store.set_machine_state(machine_id, machine_state))
        .await;
    if let Err(err) = result {
        tracing::warn!(machine = %machine_id, error = %err, "could not record machine state");
    }
}
