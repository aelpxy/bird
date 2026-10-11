use bird_core::{MachineId, MetricPoint, MetricSample, Resolution, ServiceId};
use rusqlite::{Row, params};

use crate::{Result, Store};

impl Store {
    /// Saves one round of samples, all taken at `at` (unix seconds)
    pub fn record_metrics(&mut self, at: i64, samples: &[(ServiceId, MetricSample)]) -> Result<()> {
        let at = at - at.rem_euclid(Resolution::HalfMinute.seconds());
        self.transaction(|store| {
            for (service, sample) in samples {
                let cpu = signed(sample.cpu_millicores);
                let memory = signed(sample.memory_bytes);
                store.execute(
                    "INSERT INTO metric_samples (service_id, machine_id, resolution, at,
                         cpu_millicores, cpu_peak_millicores, memory_bytes, memory_peak_bytes,
                         net_rx_bytes, net_tx_bytes)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6, ?6, ?7, ?8)
                     ON CONFLICT DO NOTHING",
                    params![
                        service.to_string(),
                        sample.machine.to_string(),
                        Resolution::HalfMinute.seconds(),
                        at,
                        cpu,
                        memory,
                        signed(sample.net_rx_bytes),
                        signed(sample.net_tx_bytes),
                    ],
                )?;
            }
            Ok(())
        })
    }

    /// Folds samples older than their resolution keeps into the next one, and drops what is past
    /// the coarsest; only whole buckets are folded, so each is written once
    pub fn roll_up_metrics(&mut self, now: i64) -> Result<()> {
        self.transaction(|store| {
            for resolution in Resolution::ALL {
                let Some(coarser) = resolution.coarser() else {
                    store.execute(
                        "DELETE FROM metric_samples WHERE resolution = ?1 AND at < ?2",
                        params![resolution.seconds(), now - resolution.kept_secs()],
                    )?;
                    continue;
                };
                let step = coarser.seconds();
                let cutoff = (now - resolution.kept_secs()).div_euclid(step) * step;
                store.execute(
                    "INSERT INTO metric_samples (service_id, machine_id, resolution, at,
                         cpu_millicores, cpu_peak_millicores, memory_bytes, memory_peak_bytes,
                         net_rx_bytes, net_tx_bytes)
                     SELECT service_id, machine_id, ?2, (at / ?2) * ?2,
                         CAST(AVG(cpu_millicores) AS INTEGER), MAX(cpu_peak_millicores),
                         CAST(AVG(memory_bytes) AS INTEGER), MAX(memory_peak_bytes),
                         SUM(net_rx_bytes), SUM(net_tx_bytes)
                     FROM metric_samples WHERE resolution = ?1 AND at < ?3
                     GROUP BY service_id, machine_id, at / ?2
                     ON CONFLICT DO NOTHING",
                    params![resolution.seconds(), step, cutoff],
                )?;
                store.execute(
                    "DELETE FROM metric_samples WHERE resolution = ?1 AND at < ?2",
                    params![resolution.seconds(), cutoff],
                )?;
            }
            Ok(())
        })
    }

    /// A service's usage since `since` in steps of `step` seconds, oldest first; each machine is
    /// averaged over the step first, then the machines are added up
    pub fn service_metrics(
        &self,
        service: ServiceId,
        since: i64,
        step: i64,
        machine: Option<MachineId>,
    ) -> Result<Vec<MetricPoint>> {
        self.query_all(
            "SELECT bucket, SUM(cpu), SUM(cpu_peak), SUM(memory), SUM(memory_peak), SUM(rx),
                    SUM(tx), COUNT(*)
             FROM (
                 SELECT machine_id, (at / ?3) * ?3 AS bucket,
                     CAST(AVG(cpu_millicores) AS INTEGER) AS cpu,
                     MAX(cpu_peak_millicores) AS cpu_peak,
                     CAST(AVG(memory_bytes) AS INTEGER) AS memory,
                     MAX(memory_peak_bytes) AS memory_peak,
                     SUM(net_rx_bytes) AS rx, SUM(net_tx_bytes) AS tx
                 FROM metric_samples
                 WHERE service_id = ?1 AND at >= ?2 AND resolution <= ?3
                     AND (?4 IS NULL OR machine_id = ?4)
                 GROUP BY machine_id, bucket
             )
             GROUP BY bucket ORDER BY bucket",
            params![
                service.to_string(),
                since,
                step,
                machine.map(|id| id.to_string())
            ],
            point,
        )
    }
}

fn point(row: &Row<'_>) -> rusqlite::Result<MetricPoint> {
    let unsigned = |idx: usize| -> rusqlite::Result<u64> {
        Ok(u64::try_from(row.get::<_, i64>(idx)?).unwrap_or(0))
    };
    Ok(MetricPoint {
        at: row.get(0)?,
        cpu_millicores: unsigned(1)?,
        cpu_peak_millicores: unsigned(2)?,
        memory_bytes: unsigned(3)?,
        memory_peak_bytes: unsigned(4)?,
        net_rx_bytes: unsigned(5)?,
        net_tx_bytes: unsigned(6)?,
        machines: u32::try_from(row.get::<_, i64>(7)?).unwrap_or(u32::MAX),
    })
}

// sqlite integers are signed; usage never comes near the top of either range
fn signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use bird_core::{MachineId, MetricSample, Resolution};

    use crate::testing::setup;

    fn sample(machine: MachineId, cpu: u64, memory: u64) -> MetricSample {
        MetricSample {
            machine,
            cpu_millicores: cpu,
            memory_bytes: memory,
            net_rx_bytes: 10,
            net_tx_bytes: 1,
        }
    }

    #[test]
    fn adds_up_machines_and_averages_within_a_step() {
        let (mut store, service) = setup();
        let (a, b) = (MachineId::generate(), MachineId::generate());
        store
            .record_metrics(
                0,
                &[
                    (service.id, sample(a, 100, 10)),
                    (service.id, sample(b, 300, 30)),
                ],
            )
            .unwrap();
        store
            .record_metrics(30, &[(service.id, sample(a, 300, 10))])
            .unwrap();
        let points = store.service_metrics(service.id, 0, 60, None).unwrap();
        assert_eq!(points.len(), 1);
        let point = points[0];
        assert_eq!(
            (point.cpu_millicores, point.cpu_peak_millicores),
            (200 + 300, 300 + 300)
        );
        assert_eq!(
            (point.memory_bytes, point.net_rx_bytes, point.machines),
            (40, 30, 2)
        );
        let only_b = store.service_metrics(service.id, 0, 60, Some(b)).unwrap();
        assert_eq!(only_b[0].cpu_millicores, 300);
    }

    #[test]
    fn rolls_old_samples_up_and_keeps_what_they_said() {
        let (mut store, service) = setup();
        let machine = MachineId::generate();
        for tick in 0..10 {
            store
                .record_metrics(
                    tick * 30,
                    &[(
                        service.id,
                        sample(machine, 100 * u64::try_from(tick).unwrap(), 5),
                    )],
                )
                .unwrap();
        }
        let before = store.service_metrics(service.id, 0, 300, None).unwrap();
        let later = Resolution::HalfMinute.kept_secs() + 600;
        store.roll_up_metrics(later).unwrap();
        let after = store.service_metrics(service.id, 0, 300, None).unwrap();
        assert_eq!(before, after);
        assert_eq!(store.service_metrics(service.id, 0, 30, None).unwrap(), []);

        store
            .roll_up_metrics(Resolution::Hour.kept_secs() + 2 * 3600)
            .unwrap();
        assert_eq!(
            store.service_metrics(service.id, 0, 3600, None).unwrap(),
            []
        );
    }
}
