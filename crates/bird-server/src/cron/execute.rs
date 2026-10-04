use bird_api::CommandEvent;
use bird_core::{CronJob, CronRunId, CronRunStatus, Service};
use bytes::Bytes;
use tokio::sync::mpsc;

use crate::state::AppState;
use crate::{Error, commands};

// the end of a run's output is kept, which is where a failure usually shows
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const KEEP_RUNS: u32 = 50;
const EVENT_BUFFER: usize = 64;

pub(super) async fn execute(
    state: &AppState,
    service: &Service,
    job: &CronJob,
    target: commands::RunTarget,
    id: CronRunId,
) {
    let (events, mut receiver) = mpsc::channel::<Bytes>(EVENT_BUFFER);
    let limit = job.timeout.duration();
    // owns the sender, so collecting ends when the command does
    let run =
        async move { commands::run_with_limit(state, &target, &job.command, &events, limit).await };
    let collect = async {
        let mut output = Tail::default();
        while let Some(line) = receiver.recv().await {
            if let Ok(CommandEvent::Output { text, .. }) = serde_json::from_slice(&line) {
                output.push(&text);
            }
        }
        output
    };
    let (result, mut output) = tokio::join!(run, collect);
    let (status, exit_code) = match result {
        Ok(Some(0)) => (CronRunStatus::Succeeded, Some(0)),
        Ok(Some(code)) => (CronRunStatus::Failed, Some(code)),
        Ok(None) | Err(Error::ShuttingDown) => (CronRunStatus::Interrupted, None),
        Err(Error::RunTimedOut(_)) => (CronRunStatus::TimedOut, None),
        Err(err) => {
            output.push(&format!("\n{err}\n"));
            (CronRunStatus::Failed, None)
        }
    };
    tracing::info!(service = %service.name, job = %job.name, run = %id, %status, ?exit_code, "cron run finished");
    let (job_id, text) = (job.id, output.0);
    let finished = state
        .db
        .call(move |store| {
            store.finish_cron_run(id, status, exit_code, &text)?;
            store.prune_cron_runs(job_id, KEEP_RUNS)
        })
        .await;
    if let Err(err) = finished {
        tracing::error!(service = %service.name, job = %job.name, run = %id, error = %err, "could not record a cron run");
    }
}

#[derive(Default)]
struct Tail(String);

impl Tail {
    fn push(&mut self, text: &str) {
        self.0.push_str(text);
        if self.0.len() > MAX_OUTPUT_BYTES {
            let mut cut = self.0.len() - MAX_OUTPUT_BYTES;
            while !self.0.is_char_boundary(cut) {
                cut += 1;
            }
            self.0.drain(..cut);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_the_end_of_long_output_whole() {
        let mut tail = Tail::default();
        tail.push("start\n");
        tail.push(&"é".repeat(MAX_OUTPUT_BYTES));
        tail.push("end\n");
        assert!(tail.0.len() <= MAX_OUTPUT_BYTES);
        assert!(tail.0.ends_with("éend\n"));
        assert!(tail.0.starts_with('é'));
    }
}
