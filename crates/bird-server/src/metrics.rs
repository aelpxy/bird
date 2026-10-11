use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use bird_api::MetricPoint;
use bird_core::MachineId;
use bird_podman::ContainerStats;
use tokio::time::{MissedTickBehavior, interval};

use crate::login::unix_now;
use crate::state::AppState;
use crate::stats;

const SAMPLE_EVERY: Duration = Duration::from_secs(30);
// an hour of samples
const KEEP_POINTS: usize = 120;

/// Recent cpu and memory of every running machine, kept in memory only
#[derive(Default)]
pub(crate) struct Metrics(Mutex<HashMap<MachineId, History>>);

struct History {
    last: ContainerStats,
    points: VecDeque<MetricPoint>,
}

impl Metrics {
    pub(crate) fn history(&self, machine: MachineId) -> Vec<MetricPoint> {
        let machines = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        machines
            .get(&machine)
            .map(|history| history.points.iter().copied().collect())
            .unwrap_or_default()
    }

    // machines missing from a sample have stopped, so their history goes with them
    fn record(&self, sampled: HashMap<MachineId, ContainerStats>, at: i64) {
        let mut machines = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        machines.retain(|machine, _| sampled.contains_key(machine));
        for (machine, now) in sampled {
            let Some(history) = machines.get_mut(&machine) else {
                machines.insert(
                    machine,
                    History {
                        last: now,
                        points: VecDeque::with_capacity(KEEP_POINTS),
                    },
                );
                continue;
            };
            if let Some(cpu_millicores) = now.cpu_millicores_since(&history.last) {
                if history.points.len() == KEEP_POINTS {
                    history.points.pop_front();
                }
                history.points.push_back(MetricPoint {
                    at,
                    cpu_millicores,
                    memory_bytes: now.memory_bytes,
                });
            }
            history.last = now;
        }
    }
}

pub(crate) async fn run(state: AppState, shutdown: impl Future<Output = ()>) {
    let mut ticker = interval(SAMPLE_EVERY);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            _ = ticker.tick() => {}
        }
        tokio::select! {
            () = &mut shutdown => break,
            () = sample(&state) => {}
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
    let containers: Vec<(MachineId, String)> = machines
        .into_iter()
        .filter_map(|machine| Some((machine.id, machine.container_id?)))
        .collect();
    let mut snapshot = stats::snapshot(state, &containers).await;
    let sampled = containers
        .into_iter()
        .filter_map(|(machine, container)| Some((machine, snapshot.remove(&container)?)))
        .collect();
    state.metrics.record(sampled, unix_now());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stats(cpu_nanos: u64, sampled_at_nanos: u64, memory_bytes: u64) -> ContainerStats {
        ContainerStats {
            cpu_nanos,
            sampled_at_nanos,
            memory_bytes,
            net_rx_bytes: 0,
            net_tx_bytes: 0,
            processes: 1,
        }
    }

    #[test]
    fn keeps_points_from_the_second_sample_and_forgets_stopped_machines() {
        let metrics = Metrics::default();
        let web = MachineId::generate();
        let worker = MachineId::generate();
        metrics.record(
            HashMap::from([(web, stats(0, 0, 10)), (worker, stats(0, 0, 5))]),
            0,
        );
        assert_eq!(metrics.history(web), []);

        let half_a_core_for_30s = 15_000_000_000;
        metrics.record(
            HashMap::from([(web, stats(half_a_core_for_30s, 30_000_000_000, 20))]),
            30,
        );
        assert_eq!(
            metrics.history(web),
            [MetricPoint {
                at: 30,
                cpu_millicores: 500,
                memory_bytes: 20
            }]
        );
        assert_eq!(metrics.history(worker), []);
    }

    #[test]
    fn keeps_an_hour() {
        let metrics = Metrics::default();
        let web = MachineId::generate();
        for tick in 0..=200_u64 {
            let at = tick * 30_000_000_000;
            metrics.record(
                HashMap::from([(web, stats(0, at, 1))]),
                i64::try_from(tick).unwrap(),
            );
        }
        let history = metrics.history(web);
        assert_eq!(history.len(), KEEP_POINTS);
        assert_eq!(history.last().unwrap().at, 200);
    }
}
