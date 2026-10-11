use std::sync::Arc;
use std::sync::atomic::{AtomicI64, Ordering};

use bird_api::DeployLogKind;
use bird_core::MachineId;
use bird_podman::LogStart;

use super::log::DeployLog;
use crate::state::AppState;

const CATCH_UP_LINES: u32 = 200;

// a new machine's output goes into the deploy log until it is healthy or has failed
pub(super) struct OutputWatch(Option<Watching>);

struct Watching {
    task: tokio::task::JoinHandle<()>,
    // podman's time of the newest line logged, so catching up repeats nothing
    seen: Arc<AtomicI64>,
    machine: MachineId,
    log: DeployLog,
}

impl OutputWatch {
    // a quick app can be healthy before the follower has read a line, so the rest is read once
    pub(super) async fn catch_up(mut self, state: &AppState, container_id: &str) {
        let Some(watching) = self.0.take() else {
            return;
        };
        watching.task.abort();
        let seen = watching.seen.load(Ordering::Relaxed);
        let lines = match state.podman.logs(container_id, CATCH_UP_LINES).await {
            Ok(lines) => lines,
            Err(err) => {
                tracing::debug!(machine = %watching.machine, error = %err, "could not read a new machine's output");
                return;
            }
        };
        for line in lines
            .into_iter()
            .filter(|line| line.at.is_none_or(|at| at > seen))
        {
            watching
                .log
                .push(DeployLogKind::App, Some(watching.machine), line.text);
        }
    }
}

impl Drop for OutputWatch {
    fn drop(&mut self) {
        if let Some(watching) = self.0.take() {
            watching.task.abort();
        }
    }
}

pub(super) fn watch_output(
    state: &AppState,
    container_id: &str,
    machine: MachineId,
    log: &DeployLog,
) -> OutputWatch {
    if !log.is_recording() {
        return OutputWatch(None);
    }
    let seen = Arc::new(AtomicI64::new(i64::MIN));
    let (state, container_id, task_log, task_seen) = (
        state.clone(),
        container_id.to_owned(),
        log.clone(),
        Arc::clone(&seen),
    );
    let task = tokio::spawn(async move {
        let mut follower = match state
            .podman
            .follow_logs(&container_id, LogStart::Beginning)
            .await
        {
            Ok(follower) => follower,
            Err(err) => {
                tracing::debug!(machine = %machine, error = %err, "could not follow a new machine's output");
                return;
            }
        };
        while let Some(Ok(line)) = follower.next().await {
            if let Some(at) = line.at {
                task_seen.store(at, Ordering::Relaxed);
            }
            task_log.push(DeployLogKind::App, Some(machine), line.text);
        }
    });
    OutputWatch(Some(Watching {
        task,
        seen,
        machine,
        log: log.clone(),
    }))
}
