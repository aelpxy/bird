use std::time::Duration;

use bird_api::CommandEvent;
use bird_core::Command;
use bytes::Bytes;
use tokio::sync::mpsc;

use super::{RunTarget, run_with_limit};
use crate::Result;
use crate::state::AppState;

// the end of a run's output is kept, which is where a failure usually shows
const MAX_OUTPUT_BYTES: usize = 64 * 1024;
const EVENT_BUFFER: usize = 64;

// runs a command in a fresh container with nobody attached, keeping the end of its output
pub(crate) async fn run_collecting(
    state: &AppState,
    target: &RunTarget,
    command: &Command,
    limit: Duration,
) -> (Result<Option<i32>>, String) {
    let (events, mut receiver) = mpsc::channel::<Bytes>(EVENT_BUFFER);
    // owns the sender, so collecting ends when the command does
    let run = async move { run_with_limit(state, target, command, &events, limit).await };
    let collect = async {
        let mut output = Tail::default();
        while let Some(line) = receiver.recv().await {
            if let Ok(CommandEvent::Output { text, .. }) = serde_json::from_slice(&line) {
                output.push(&text);
            }
        }
        output
    };
    let (result, output) = tokio::join!(run, collect);
    (result, output.0)
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
