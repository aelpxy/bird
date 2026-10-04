use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddr};

use bird_core::{Machine, MachineId, MachineState, Port, Service};
use bird_podman::ContainerState;

use super::verdict::{MAX_STRIKES, Verdict, judge};
use crate::deploy::set_state;
use crate::state::AppState;
use crate::{Result, health};

pub(super) async fn check_machine(
    state: &AppState,
    service: &Service,
    machine: &Machine,
    port: Port,
    strikes: &mut HashMap<MachineId, u32>,
) -> Result<Verdict> {
    let Some(container_id) = machine.container_id.as_deref() else {
        fail(state, service, machine, strikes, "machine has no container").await;
        return Ok(Verdict::Fail);
    };
    let info = match state.podman.inspect_container(container_id).await {
        Ok(info) => info,
        Err(bird_podman::Error::NotFound { .. }) => {
            fail(state, service, machine, strikes, "container is gone").await;
            return Ok(Verdict::Fail);
        }
        Err(err) => return Err(err.into()),
    };

    let address = info
        .host_port(port)
        .map(|host_port| SocketAddr::from((Ipv4Addr::LOCALHOST, host_port)));
    let responded = match (info.state, address) {
        (ContainerState::Running, Some(address)) => {
            Some(health::probe(&service.health, address).await)
        }
        _ => None,
    };
    let previous = strikes.get(&machine.id).copied().unwrap_or(0);
    let verdict = judge(info.state, responded, previous);

    match verdict {
        Verdict::Healthy => {
            strikes.remove(&machine.id);
            if let Some(address) = address.filter(|a| machine.address != Some(*a)) {
                let id = machine.id;
                state
                    .db
                    .call(move |store| store.set_machine_address(id, address))
                    .await?;
            }
        }
        Verdict::Start { strikes: count } => {
            strikes.insert(machine.id, count);
            if info.state == ContainerState::Paused {
                // a backup interrupted by a birdd restart leaves its machine paused
                tracing::warn!(service = %service.name, machine = %machine.id, "machine was left paused, resuming it");
                if let Err(err) = state.podman.unpause_container(container_id).await {
                    tracing::warn!(machine = %machine.id, error = %err, "could not resume container");
                }
                return Ok(verdict);
            }
            if info.oom_killed {
                tracing::warn!(service = %service.name, machine = %machine.id, memory = %service.memory, "machine ran out of memory, starting it again");
            } else {
                tracing::warn!(service = %service.name, machine = %machine.id, strikes = count, max = MAX_STRIKES, "container stopped, starting it");
            }
            if let Err(err) = state.podman.start_container(container_id).await {
                tracing::warn!(machine = %machine.id, error = %err, "could not start container");
            }
        }
        Verdict::Strike { strikes: count } => {
            strikes.insert(machine.id, count);
            tracing::warn!(service = %service.name, machine = %machine.id, strikes = count, max = MAX_STRIKES, "health check failed");
        }
        Verdict::Fail => {
            fail(
                state,
                service,
                machine,
                strikes,
                "health checks kept failing",
            )
            .await;
        }
    }
    Ok(verdict)
}

async fn fail(
    state: &AppState,
    service: &Service,
    machine: &Machine,
    strikes: &mut HashMap<MachineId, u32>,
    reason: &str,
) {
    strikes.remove(&machine.id);
    tracing::error!(service = %service.name, machine = %machine.id, reason, "machine failed, it will be replaced");
    set_state(state, machine.id, MachineState::Failed).await;
}
