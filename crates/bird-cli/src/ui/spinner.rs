use std::io::{IsTerminal, Write};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use tokio::task::JoinHandle;

use super::style::{self, Paint};

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];
const FRAME_INTERVAL: Duration = Duration::from_millis(80);
const CLEAR_LINE: &str = "\r\x1b[2K";

// animates on stderr while a long request runs; without a terminal it prints the message once
pub(crate) struct Spinner {
    message: Arc<Mutex<String>>,
    task: Option<JoinHandle<()>>,
    started: Instant,
}

impl Spinner {
    pub(crate) fn start(message: impl Into<String>) -> Self {
        let message = Arc::new(Mutex::new(message.into()));
        let started = Instant::now();
        if !std::io::stderr().is_terminal() {
            eprintln!("{}...", lock(&message));
            return Self {
                message,
                task: None,
                started,
            };
        }
        let shown = Arc::clone(&message);
        let task = tokio::spawn(async move {
            let mut ticks = tokio::time::interval(FRAME_INTERVAL);
            for frame in FRAMES.iter().cycle() {
                ticks.tick().await;
                let line = format!(
                    "{CLEAR_LINE}{} {} {}",
                    style::err(Paint::Cyan, frame),
                    lock(&shown),
                    style::err(Paint::Dim, duration(started.elapsed()))
                );
                let mut stderr = std::io::stderr().lock();
                let _ = stderr.write_all(line.as_bytes());
                let _ = stderr.flush();
            }
        });
        Self {
            message,
            task: Some(task),
            started,
        }
    }

    pub(crate) fn set(&self, message: impl Into<String>) {
        *lock(&self.message) = message.into();
    }

    pub(crate) fn elapsed(&self) -> Duration {
        self.started.elapsed()
    }
}

// the runtime is single threaded, so the animation is never mid-write when this runs
impl Drop for Spinner {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
            let mut stderr = std::io::stderr().lock();
            let _ = stderr.write_all(CLEAR_LINE.as_bytes());
            let _ = stderr.flush();
        }
    }
}

fn lock(message: &Mutex<String>) -> std::sync::MutexGuard<'_, String> {
    message.lock().unwrap_or_else(PoisonError::into_inner)
}

#[must_use]
pub(crate) fn duration(elapsed: Duration) -> String {
    let secs = elapsed.as_secs();
    if secs < 60 {
        format!("{secs}s")
    } else {
        format!("{}m{:02}s", secs / 60, secs % 60)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(duration(Duration::from_millis(900)), "0s");
        assert_eq!(duration(Duration::from_secs(59)), "59s");
        assert_eq!(duration(Duration::from_secs(125)), "2m05s");
    }
}
