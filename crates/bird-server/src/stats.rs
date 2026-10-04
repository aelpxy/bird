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
    let ids: Vec<&str> = containers.iter().map(|(_, id)| id.as_str()).collect();
    let pair = async {
        let first = state.podman.stats(&ids).await?;
        tokio::time::sleep(SAMPLE_WINDOW).await;
        let second = state.podman.stats(&ids).await?;
        Ok::<_, bird_podman::Error>((first, second))
    };
    let (first, second) = match pair.await {
        Ok(pair) => pair,
        Err(err) => {
            tracing::warn!(error = %err, "could not sample machine stats");
            return BTreeMap::new();
        }
    };
    containers
        .iter()
        .filter_map(|(machine, id)| {
            let usage = combine(first.get(id)?, second.get(id)?)?;
            Some((*machine, usage))
        })
        .collect()
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
