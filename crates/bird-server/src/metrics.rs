use std::collections::HashMap;
use std::future::Future;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use bird_core::{MachineId, MetricSample, ServiceId};
use bird_podman::ContainerStats;
use tokio::time::{MissedTickBehavior, interval};

use crate::login::unix_now;
use crate::state::AppState;
use crate::stats;

const SAMPLE_EVERY: Duration = Duration::from_secs(30);
// rolling up every ten samples keeps the half-minute rows from piling up past their six hours
const ROLL_UP_EVERY_TICKS: u32 = 10;

/// Each running machine's previous reading, since cpu and traffic are counted between two
#[derive(Default)]
pub(crate) struct Metrics(Mutex<HashMap<MachineId, ContainerStats>>);

impl Metrics {
    // machines missing from a round have stopped, so their reading goes with them
    fn advance(
        &self,
        readings: Vec<(ServiceId, MachineId, ContainerStats)>,
    ) -> Vec<(ServiceId, MetricSample)> {
        let mut last = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mut next = HashMap::with_capacity(readings.len());
        let samples = readings
            .into_iter()
            .filter_map(|(service, machine, now)| {
                next.insert(machine, now);
                let before = last.get(&machine)?;
                Some((
                    service,
                    MetricSample {
                        machine,
                        cpu_millicores: now.cpu_millicores_since(before)?,
                        memory_bytes: now.memory_bytes,
                        // a restarted container counts from zero again
                        net_rx_bytes: now
                            .net_rx_bytes
                            .checked_sub(before.net_rx_bytes)
                            .unwrap_or(now.net_rx_bytes),
                        net_tx_bytes: now
                            .net_tx_bytes
                            .checked_sub(before.net_tx_bytes)
                            .unwrap_or(now.net_tx_bytes),
                    },
                ))
            })
            .collect();
        *last = next;
        samples
    }
}

pub(crate) async fn run(state: AppState, shutdown: impl Future<Output = ()>) {
    let mut ticker = interval(SAMPLE_EVERY);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    tokio::pin!(shutdown);
    let mut ticks = 0_u32;
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            _ = ticker.tick() => {}
        }
        tokio::select! {
            () = &mut shutdown => break,
            () = sample(&state) => {}
        }
        ticks = ticks.wrapping_add(1);
        if ticks.is_multiple_of(ROLL_UP_EVERY_TICKS) {
            let now = unix_now();
            if let Err(err) = state.db.call(move |store| store.roll_up_metrics(now)).await {
                tracing::warn!(error = %err, "could not roll up usage history");
            }
        }
    }
    tracing::info!("metrics sampler stopped");
}

async fn sample(state: &AppState) {
    let machines = match state.db.call(|store| store.list_running_machines()).await {
        Ok(machines) => machines,
        Err(err) => {
            tracing::warn!(error = %err, "could not list machines to sample");
            return;
        }
    };
    let found: Vec<(ServiceId, MachineId, String)> = machines
        .into_iter()
        .filter_map(|(service, machine)| Some((service, machine.id, machine.container_id?)))
        .collect();
    let containers: Vec<(MachineId, String)> = found
        .iter()
        .map(|(_, machine, container)| (*machine, container.clone()))
        .collect();
    let mut snapshot = stats::snapshot(state, &containers).await;
    let readings = found
        .into_iter()
        .filter_map(|(service, machine, container)| {
            Some((service, machine, snapshot.remove(&container)?))
        })
        .collect();
    let samples = state.metrics.advance(readings);
    if samples.is_empty() {
        return;
    }
    let at = unix_now();
    if let Err(err) = state
        .db
        .call(move |store| store.record_metrics(at, &samples))
        .await
    {
        tracing::warn!(error = %err, "could not save usage samples");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(cpu_nanos: u64, sampled_at_nanos: u64, net_rx_bytes: u64) -> ContainerStats {
        ContainerStats {
            cpu_nanos,
            sampled_at_nanos,
            memory_bytes: 20,
            net_rx_bytes,
            net_tx_bytes: 0,
            processes: 1,
        }
    }

    #[test]
    fn counts_cpu_and_traffic_between_readings_and_forgets_stopped_machines() {
        let metrics = Metrics::default();
        let (service, web, worker) = (
            ServiceId::generate(),
            MachineId::generate(),
            MachineId::generate(),
        );
        let first = metrics.advance(vec![
            (service, web, stats(0, 0, 100)),
            (service, worker, stats(0, 0, 0)),
        ]);
        assert_eq!(first, []);

        let half_a_core_for_30s = 15_000_000_000;
        let second = metrics.advance(vec![(
            service,
            web,
            stats(half_a_core_for_30s, 30_000_000_000, 150),
        )]);
        assert_eq!(
            second,
            [(
                service,
                MetricSample {
                    machine: web,
                    cpu_millicores: 500,
                    memory_bytes: 20,
                    net_rx_bytes: 50,
                    net_tx_bytes: 0
                }
            )]
        );
        let back = metrics.advance(vec![(service, worker, stats(0, 30_000_000_000, 0))]);
        assert_eq!(back, [], "a machine that missed a round starts over");
    }

    #[test]
    fn a_restarted_counter_counts_from_zero() {
        let metrics = Metrics::default();
        let (service, web) = (ServiceId::generate(), MachineId::generate());
        metrics.advance(vec![(service, web, stats(0, 0, 900))]);
        let after = metrics.advance(vec![(service, web, stats(1, 30_000_000_000, 40))]);
        assert_eq!(after[0].1.net_rx_bytes, 40);
    }
}
