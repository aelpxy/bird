use std::collections::BTreeMap;
use std::time::Duration;

use bird_api::MachineStats;
use bird_core::MachineId;
use bird_podman::ContainerStats;

use crate::state::AppState;

// long enough to see load, short enough that `bird status` still feels instant
const SAMPLE_WINDOW: Duration = Duration::from_millis(500);

// stats only add to a status, so failing to read them is logged instead of failing the request
pub(crate) async fn sample(
    state: &AppState,
    containers: &[(MachineId, String)],
) -> BTreeMap<MachineId, MachineStats> {
    let first = snapshot(state, containers).await;
    if first.is_empty() {
        return BTreeMap::new();
    }
    tokio::time::sleep(SAMPLE_WINDOW).await;
    let second = snapshot(state, containers).await;
    containers
        .iter()
        .filter_map(|(machine, id)| {
            let usage = combine(first.get(id)?, second.get(id)?)?;
            Some((*machine, usage))
        })
        .collect()
}

// one container at a time: podman fails the whole request when any of them was removed meanwhile
async fn snapshot(
    state: &AppState,
    containers: &[(MachineId, String)],
) -> BTreeMap<String, ContainerStats> {
    let mut found = BTreeMap::new();
    for (machine, id) in containers {
        match state.podman.stats(&[id]).await {
            Ok(sampled) => found.extend(sampled),
            Err(bird_podman::Error::NotFound { .. }) => {}
            Err(err) => {
                tracing::warn!(machine = %machine, error = %err, "could not sample machine stats");
            }
        }
    }
    found
}

fn combine(first: &ContainerStats, second: &ContainerStats) -> Option<MachineStats> {
    Some(MachineStats {
        cpu_millicores: second.cpu_millicores_since(first)?,
        memory_bytes: second.memory_bytes,
        net_rx_bytes: second.net_rx_bytes,
        net_tx_bytes: second.net_tx_bytes,
        processes: second.processes,
    })
}
