mod exec;
mod piped;
mod run;
pub(crate) mod tty;

use bird_api::{CommandEvent, LogStream};
use bird_podman::{OutputChunk, OutputFollower};
use bytes::Bytes;
use serde::Serialize;
use tokio::sync::mpsc;

pub(crate) use exec::{exec, pick_machine};
pub(crate) use run::{remove_leftover_runs, run, run_target, start_attached};

use crate::Result;

// copies a command's output to the client until it ends; false when the client went away
async fn forward(output: &mut OutputFollower, events: &mpsc::Sender<Bytes>) -> Result<bool> {
    while let Some(chunk) = output.next().await {
        if !send(events, &event(chunk?)).await {
            return Ok(false);
        }
    }
    Ok(true)
}

fn event(chunk: OutputChunk) -> CommandEvent {
    CommandEvent::Output {
        stream: match chunk.stream {
            bird_podman::LogStream::Stdout => LogStream::Stdout,
            bird_podman::LogStream::Stderr => LogStream::Stderr,
        },
        text: chunk.text,
    }
}

pub(crate) async fn send(events: &mpsc::Sender<Bytes>, event: &impl Serialize) -> bool {
    let Ok(mut json) = serde_json::to_vec(event) else {
        return false;
    };
    json.push(b'\n');
    events.send(Bytes::from(json)).await.is_ok()
}
