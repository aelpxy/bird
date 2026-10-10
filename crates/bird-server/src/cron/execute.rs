use bird_core::{CronJob, CronRunId, CronRunStatus, Service};

use crate::state::AppState;
use crate::{Error, commands};

const KEEP_RUNS: u32 = 50;

pub(super) async fn execute(
    state: &AppState,
    service: &Service,
    job: &CronJob,
    target: commands::RunTarget,
    id: CronRunId,
) {
    let (result, mut output) =
        commands::run_collecting(state, &target, &job.command, job.timeout.duration()).await;
    let (status, exit_code) = match result {
        Ok(Some(0)) => (CronRunStatus::Succeeded, Some(0)),
        Ok(Some(code)) => (CronRunStatus::Failed, Some(code)),
        Ok(None) | Err(Error::ShuttingDown) => (CronRunStatus::Interrupted, None),
        Err(Error::RunTimedOut(_)) => (CronRunStatus::TimedOut, None),
        Err(err) => {
            output.push('\n');
            output.push_str(&err.to_string());
            output.push('\n');
            (CronRunStatus::Failed, None)
        }
    };
    tracing::info!(service = %service.name, job = %job.name, run = %id, %status, ?exit_code, "cron run finished");
    let job_id = job.id;
    let finished = state
        .db
        .call(move |store| {
            store.finish_cron_run(id, status, exit_code, &output)?;
            store.prune_cron_runs(job_id, KEEP_RUNS)
        })
        .await;
    if let Err(err) = finished {
        tracing::error!(service = %service.name, job = %job.name, run = %id, error = %err, "could not record a cron run");
    }
}
