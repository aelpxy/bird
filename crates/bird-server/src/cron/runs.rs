use std::collections::HashSet;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use bird_core::{CronJob, CronJobId, CronRun, CronRunStatus, CronTrigger, Service};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::execute::execute;
use crate::state::AppState;
use crate::{Error, Result, commands};

// each run is a container, so a burst of jobs cannot take over the server
pub(crate) const MAX_RUNNING: usize = 16;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(15);

// jobs running now, so a job never overlaps itself, and a slot per run that shutdown waits for
pub(crate) struct CronRuns {
    running: Mutex<HashSet<CronJobId>>,
    slots: Arc<Semaphore>,
}

impl Default for CronRuns {
    fn default() -> Self {
        Self {
            running: Mutex::new(HashSet::new()),
            slots: Arc::new(Semaphore::new(MAX_RUNNING)),
        }
    }
}

impl CronRuns {
    fn claim(self: &Arc<Self>, job: &CronJob) -> Result<Claim> {
        let slot = Arc::clone(&self.slots)
            .try_acquire_owned()
            .map_err(|_| Error::TooManyCronRuns)?;
        let mut running = self.running.lock().unwrap_or_else(PoisonError::into_inner);
        if !running.insert(job.id) {
            return Err(Error::CronJobRunning(job.name.clone()));
        }
        Ok(Claim {
            runs: Arc::clone(self),
            job: job.id,
            _slot: slot,
        })
    }

    pub(crate) async fn wait_for_runs(&self) {
        let all = u32::try_from(MAX_RUNNING).unwrap_or(u32::MAX);
        if tokio::time::timeout(SHUTDOWN_WAIT, self.slots.acquire_many(all))
            .await
            .is_err()
        {
            tracing::warn!("some cron runs were still ending when birdd stopped");
        }
    }
}

// held by the task running a job, so the job is free again however the task ends
pub(super) struct Claim {
    runs: Arc<CronRuns>,
    job: CronJobId,
    _slot: OwnedSemaphorePermit,
}

impl Drop for Claim {
    fn drop(&mut self) {
        self.runs
            .running
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&self.job);
    }
}

// starts the job in the background and returns its run; a scheduled job that cannot start is
// recorded as skipped, a manual one answers with the error
pub(crate) async fn start(
    state: &AppState,
    service: &Service,
    job: &CronJob,
    trigger: CronTrigger,
) -> Result<CronRun> {
    let prepared = match state.crons.claim(job) {
        Ok(claim) => commands::run_target(state, service.environment_id, &service.name)
            .await
            .map(|target| (claim, target)),
        Err(err) => Err(err),
    };
    let (claim, target) = match prepared {
        Ok(prepared) => prepared,
        Err(err) if trigger == CronTrigger::Schedule => {
            tracing::warn!(service = %service.name, job = %job.name, error = %err, "skipped a cron run");
            return skip(state, job, err.to_string()).await;
        }
        Err(err) => return Err(err),
    };
    let job_id = job.id;
    let run = state
        .db
        .call(move |store| store.start_cron_run(job_id, trigger, CronRunStatus::Running, ""))
        .await?;
    tracing::info!(service = %service.name, job = %job.name, run = %run.id, %trigger, "cron run started");
    let (state, service, job, id) = (state.clone(), service.clone(), job.clone(), run.id);
    tokio::spawn(async move {
        execute(&state, &service, &job, target, id).await;
        drop(claim);
    });
    Ok(run)
}

// a scheduled time that passed without a run, with the reason as its output
pub(super) async fn skip(state: &AppState, job: &CronJob, reason: String) -> Result<CronRun> {
    let job_id = job.id;
    state
        .db
        .call(move |store| {
            store.start_cron_run(
                job_id,
                CronTrigger::Schedule,
                CronRunStatus::Skipped,
                &reason,
            )
        })
        .await
}

// runs a birdd that stopped without recording them was in the middle of
pub(crate) async fn interrupt_left_over(state: &AppState) -> Result<()> {
    let interrupted = state.db.call(|store| store.interrupt_cron_runs()).await?;
    if interrupted > 0 {
        tracing::warn!(
            runs = interrupted,
            "cron runs were cut short when birdd last stopped"
        );
    }
    Ok(())
}
