use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use bird_api::{DeployLogKind, DeployLogLine};
use bird_core::{DeploymentId, MachineId, ServiceId};
use tokio::sync::broadcast;

use crate::state::AppState;

// a deploy's log lives in memory until it ends; the newest lines win when it grows past this
const MAX_LINES: usize = 3000;
const MAX_LINE_CHARS: usize = 2000;
// what one deploy may keep in memory and in bird.db, however long its lines
const MAX_BYTES: usize = 1024 * 1024;
const LIVE_BUFFER: usize = 1024;

/// What one deploy did, as it happens; cloning shares it
#[derive(Clone, Default)]
pub(crate) struct DeployLog(Option<Arc<Shared>>);

struct Shared {
    lines: Mutex<Lines>,
    live: broadcast::Sender<DeployLogLine>,
}

#[derive(Default)]
struct Lines {
    next: u32,
    kept: VecDeque<DeployLogLine>,
    bytes: usize,
}

// deploys still running, so a follower can catch up on them before they are saved
pub(crate) type RunningLogs = Mutex<HashMap<DeploymentId, DeployLog>>;

impl DeployLog {
    pub(crate) fn new() -> Self {
        let (live, _) = broadcast::channel(LIVE_BUFFER);
        Self(Some(Arc::new(Shared {
            lines: Mutex::default(),
            live,
        })))
    }

    /// For machines the supervisor replaces, which nobody watches
    pub(crate) const fn discard() -> Self {
        Self(None)
    }

    pub(crate) const fn is_recording(&self) -> bool {
        self.0.is_some()
    }

    pub(crate) fn bird(&self, text: impl Into<String>) {
        self.push(DeployLogKind::Bird, None, text.into());
    }

    pub(crate) fn push(&self, kind: DeployLogKind, machine: Option<MachineId>, text: String) {
        let Some(shared) = &self.0 else {
            return;
        };
        let text = truncate(text);
        let mut lines = shared.lines.lock().unwrap_or_else(PoisonError::into_inner);
        let line = DeployLogLine {
            seq: lines.next,
            at: unix_millis(),
            kind,
            machine,
            text,
        };
        lines.next = lines.next.saturating_add(1);
        lines.bytes += line.text.len();
        lines.kept.push_back(line.clone());
        while lines.kept.len() > MAX_LINES || lines.bytes > MAX_BYTES {
            let Some(dropped) = lines.kept.pop_front() else {
                break;
            };
            lines.bytes -= dropped.text.len();
        }
        // nobody listening is fine, the line is kept for later
        let _ = shared.live.send(line);
    }

    /// The lines so far and every one after, with no gap between them
    pub(crate) fn follow(
        &self,
    ) -> (
        Vec<DeployLogLine>,
        Option<broadcast::Receiver<DeployLogLine>>,
    ) {
        let Some(shared) = &self.0 else {
            return (Vec::new(), None);
        };
        let lines = shared.lines.lock().unwrap_or_else(PoisonError::into_inner);
        let receiver = shared.live.subscribe();
        (lines.kept.iter().cloned().collect(), Some(receiver))
    }

    pub(crate) fn subscribe(&self) -> Option<broadcast::Receiver<DeployLogLine>> {
        self.0.as_ref().map(|shared| shared.live.subscribe())
    }

    pub(crate) fn register(&self, state: &AppState, deployment: DeploymentId) {
        if self.0.is_some() {
            running(state).insert(deployment, self.clone());
        }
    }

    /// Saves the log with its deployment and stops listing it as running
    pub(crate) async fn finish(
        &self,
        state: &AppState,
        service: ServiceId,
        deployment: DeploymentId,
    ) {
        let Some(shared) = &self.0 else {
            return;
        };
        let lines: Vec<DeployLogLine> = shared
            .lines
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .kept
            .iter()
            .cloned()
            .collect();
        match serde_json::to_string(&lines) {
            Ok(encoded) => {
                let saved = state
                    .db
                    .call(move |store| store.put_deployment_log(service, deployment, &encoded))
                    .await;
                if let Err(err) = saved {
                    tracing::warn!(deployment = %deployment, error = %err, "could not save the deploy log");
                }
            }
            Err(err) => {
                tracing::warn!(deployment = %deployment, error = %err, "could not encode the deploy log");
            }
        }
        running(state).remove(&deployment);
    }
}

pub(crate) fn running(
    state: &AppState,
) -> std::sync::MutexGuard<'_, HashMap<DeploymentId, DeployLog>> {
    state
        .deploy_logs
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

// splits output that arrives in chunks into whole lines
#[derive(Default)]
pub(crate) struct LineSplitter(String);

impl LineSplitter {
    pub(crate) fn feed(&mut self, chunk: &str, mut line: impl FnMut(String)) {
        self.0.push_str(chunk);
        while let Some(end) = self.0.find('\n') {
            let rest = self.0.split_off(end + 1);
            let whole = std::mem::replace(&mut self.0, rest);
            line(whole.trim_end_matches(['\n', '\r']).to_owned());
        }
    }

    pub(crate) fn finish(self, mut line: impl FnMut(String)) {
        if !self.0.is_empty() {
            line(self.0);
        }
    }
}

fn truncate(mut text: String) -> String {
    if text.len() > MAX_LINE_CHARS {
        let mut cut = MAX_LINE_CHARS;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push('…');
    }
    text
}

fn unix_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_lines_and_lets_a_late_follower_catch_up() {
        let log = DeployLog::new();
        log.bird("pulling");
        let (before, receiver) = log.follow();
        log.push(DeployLogKind::Release, None, "migrating".to_owned());
        assert_eq!(before.len(), 1);
        assert_eq!(before[0].seq, 0);
        let next = receiver.unwrap().try_recv().unwrap();
        assert_eq!((next.seq, next.text.as_str()), (1, "migrating"));
    }

    #[test]
    fn keeps_the_newest_lines_and_shortens_long_ones() {
        let log = DeployLog::new();
        for n in 0..MAX_LINES + 5 {
            log.bird(n.to_string());
        }
        log.bird("x".repeat(MAX_LINE_CHARS + 10));
        let (kept, _) = log.follow();
        assert_eq!(kept.len(), MAX_LINES);
        assert_eq!(kept[0].text, "6");
        assert!(kept.last().unwrap().text.ends_with('…'));

        let wide = DeployLog::new();
        for _ in 0..MAX_LINES {
            wide.bird("x".repeat(MAX_LINE_CHARS));
        }
        let (kept, _) = wide.follow();
        assert!(kept.iter().map(|line| line.text.len()).sum::<usize>() <= MAX_BYTES);
        assert_eq!(kept.len(), MAX_BYTES / MAX_LINE_CHARS);
        assert_eq!(DeployLog::discard().follow().0, Vec::new());
    }

    #[test]
    fn splits_chunks_into_lines() {
        let mut splitter = LineSplitter::default();
        let mut lines = Vec::new();
        splitter.feed("one\ntw", |line| lines.push(line));
        splitter.feed("o\r\nthree", |line| lines.push(line));
        splitter.finish(|line| lines.push(line));
        assert_eq!(lines, ["one", "two", "three"]);
    }
}
