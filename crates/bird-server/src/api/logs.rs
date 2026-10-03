use std::future::Future;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use bird_api::{LogEntry, LogStream};
use bird_core::{MachineId, MachineState, Name};
use bird_podman::{LogLine, Podman};
use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::stream::ChannelBody;
use crate::state::AppState;
use crate::{Error, Result};

const DEFAULT_TAIL: u32 = 100;
const MAX_TAIL: u32 = 10_000;
const STREAM_BUFFER: usize = 256;

#[derive(Deserialize)]
pub(crate) struct LogsQuery {
    tail: Option<u32>,
    #[serde(default)]
    follow: bool,
}

pub(crate) async fn logs(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<LogsQuery>,
) -> Result<Response> {
    let machines = running_machines(&state, &name).await?;
    let tail = query.tail.unwrap_or(DEFAULT_TAIL).min(MAX_TAIL);
    if query.follow {
        return Ok(follow(&state, machines, tail));
    }
    let mut entries = Vec::new();
    for (machine, container) in machines {
        let lines = state.podman.logs(&container, tail).await?;
        entries.extend(lines.into_iter().map(|line| entry(machine, line)));
    }
    Ok(Json(entries).into_response())
}

async fn running_machines(state: &AppState, name: &Name) -> Result<Vec<(MachineId, String)>> {
    let service = state.service(name).await?;
    let service_id = service.id;
    let machines = state
        .db
        .call(move |store| {
            let Some(deployment) = store.active_deployment(service_id)? else {
                return Ok(Vec::new());
            };
            store.list_machines(deployment.id)
        })
        .await?;
    let running: Vec<(MachineId, String)> = machines
        .into_iter()
        .filter(|m| m.state == MachineState::Running)
        .filter_map(|m| Some((m.id, m.container_id?)))
        .collect();
    if running.is_empty() {
        return Err(Error::NoMachines(name.clone()));
    }
    Ok(running)
}

fn follow(state: &AppState, machines: Vec<(MachineId, String)>, tail: u32) -> Response {
    let (lines, receiver) = mpsc::channel(STREAM_BUFFER);
    for (machine, container) in machines {
        tokio::spawn(forward(
            state.podman.clone(),
            machine,
            container,
            tail,
            lines.clone(),
            state.shutdown.wait(),
        ));
    }
    (
        [(CONTENT_TYPE, "application/x-ndjson")],
        Body::new(ChannelBody(receiver)),
    )
        .into_response()
}

async fn forward(
    podman: Podman,
    machine: MachineId,
    container: String,
    tail: u32,
    lines: mpsc::Sender<Bytes>,
    shutdown: impl Future<Output = ()>,
) {
    let mut follower = match podman.follow_logs(&container, tail).await {
        Ok(follower) => follower,
        Err(err) => {
            tracing::warn!(%machine, error = %err, "could not follow logs");
            return;
        }
    };
    tokio::pin!(shutdown);
    loop {
        let line = tokio::select! {
            () = &mut shutdown => break,
            () = lines.closed() => break,
            next = follower.next() => match next {
                Some(Ok(line)) => line,
                Some(Err(err)) => {
                    tracing::debug!(%machine, error = %err, "log stream ended");
                    break;
                }
                None => break,
            },
        };
        let Ok(mut json) = serde_json::to_vec(&entry(machine, line)) else {
            break;
        };
        json.push(b'\n');
        if lines.send(Bytes::from(json)).await.is_err() {
            break;
        }
    }
}

fn entry(machine: MachineId, line: LogLine) -> LogEntry {
    LogEntry {
        machine,
        stream: match line.stream {
            bird_podman::LogStream::Stdout => LogStream::Stdout,
            bird_podman::LogStream::Stderr => LogStream::Stderr,
        },
        text: line.text,
    }
}
