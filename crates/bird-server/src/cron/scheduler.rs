use std::future::Future;
use std::time::Duration;

use bird_core::{CronJob, CronSchedule, CronTrigger, ServiceState};
use tokio::time::{MissedTickBehavior, interval};

use super::runs::{skip, start};
use crate::login::unix_now;
use crate::state::AppState;
use crate::{Error, Result};

const TICK: Duration = Duration::from_secs(10);
// a time missed by more than this, like while birdd was down, is dropped instead of run late
const MAX_LATE_SECS: i64 = 120;

pub(crate) struct CronScheduler {
    state: AppState,
}

impl CronScheduler {
    pub(crate) fn new(state: AppState) -> Self {
        Self { state }
    }

    pub(crate) async fn run(self, shutdown: impl Future<Output = ()>) {
        let mut ticker = interval(TICK);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                _ = ticker.tick() => {}
            }
            tokio::select! {
                () = &mut shutdown => break,
                result = self.tick() => if let Err(err) = result {
                    tracing::warn!(error = %err, "could not check cron jobs");
                },
            }
        }
        tracing::info!("cron scheduler stopped");
    }

    async fn tick(&self) -> Result<()> {
        let jobs = self.state.db.call(|store| store.all_cron_jobs()).await?;
        let now = unix_now();
        for job in jobs {
            let Some(due_at) = due(
                &job.schedule,
                job.last_due_at.unwrap_or(job.created_at),
                now,
            ) else {
                continue;
            };
            if let Err(err) = self.start_due(&job, due_at).await {
                tracing::error!(job = %job.name, job_id = %job.id, error = %err, "could not start a cron run");
            }
        }
        Ok(())
    }

    async fn start_due(&self, job: &CronJob, due_at: i64) -> Result<()> {
        let state = &self.state;
        let (job_id, service_id) = (job.id, job.service_id);
        let service = state
            .db
            .call(move |store| {
                if !store.claim_cron_due(job_id, due_at)? {
                    return Ok(None);
                }
                store.service(service_id)
            })
            .await?;
        let Some(service) = service else {
            return Ok(());
        };
        match service.state {
            ServiceState::Running => start(state, &service, job, CronTrigger::Schedule).await?,
            ServiceState::Stopped => {
                skip(state, job, Error::ServiceStopped(service.name).to_string()).await?
            }
        };
        Ok(())
    }
}

// the latest scheduled time after `handled` that has come, unless it is too late to run
#[must_use]
fn due(schedule: &CronSchedule, handled: i64, now: i64) -> Option<i64> {
    let mut at = handled.max(now.saturating_sub(MAX_LATE_SECS));
    let mut latest = None;
    while let Some(next) = schedule.next_after(at).filter(|&next| next <= now) {
        latest = Some(next);
        at = next;
    }
    latest
}

#[cfg(test)]
mod tests {
    use super::*;

    // 2026-10-04 10:07:30 UTC
    const NOW: i64 = 1_791_108_450;
    const MINUTE: i64 = 60;

    fn schedule(text: &str) -> CronSchedule {
        text.parse().unwrap()
    }

    #[test]
    fn nothing_is_due_before_the_first_time() {
        let every_quarter = schedule("*/15 * * * *");
        assert_eq!(due(&every_quarter, NOW - 30, NOW), None);
        assert_eq!(due(&every_quarter, NOW, NOW + 7 * MINUTE), None);
    }

    #[test]
    fn a_time_that_has_come_is_due_once() {
        let every_minute = schedule("* * * * *");
        let at = NOW + 30;
        assert_eq!(due(&every_minute, NOW, at), Some(at));
        assert_eq!(due(&every_minute, at, at + 59), None);
        assert_eq!(due(&every_minute, at, at + 60), Some(at + 60));
    }

    #[test]
    fn several_times_passed_at_once_run_only_the_latest() {
        let every_minute = schedule("* * * * *");
        let handled = NOW - 30 - 2 * MINUTE;
        assert_eq!(due(&every_minute, handled, NOW + 30), Some(NOW + 30));
    }

    #[test]
    fn times_missed_too_long_ago_are_dropped() {
        let hourly = schedule("0 * * * *");
        let ten = NOW - 7 * MINUTE - 30;
        assert_eq!(due(&hourly, ten - 1, ten + 30), Some(ten));
        assert_eq!(due(&hourly, ten - 1, NOW), None);
        assert_eq!(due(&hourly, ten - 1, ten + 50 * MINUTE + 3600), None);
        assert_eq!(due(&hourly, ten - 1, ten + 3600 + 30), Some(ten + 3600));
    }
}
