use bird_api::ErrorBody;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, SystemTime};

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use bird_api::{LogEntry, LogStream};
use bird_core::{MachineId, MachineState, Name};
use bird_podman::{LogLine, LogStart, Podman};
use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio::task::JoinSet;

use super::stream::ChannelBody;
use crate::state::AppState;
use crate::{Error, Result};

const DEFAULT_TAIL: u32 = 100;
const MAX_TAIL: u32 = 10_000;
const STREAM_BUFFER: usize = 256;
const MACHINE_POLL: Duration = Duration::from_secs(1);

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct LogsQuery {
    /// Number of recent lines per machine, default 100, at most 10000
    tail: Option<u32>,
    /// Keep the response open and stream new lines as they are written, also from machines that
    /// start later, like replacements from a deploy
    #[serde(default)]
    follow: bool,
}

/// Read or follow logs
#[utoipa::path(get, path = "/v1/services/{name}/logs", tag = "logs", params(("name" = String, Path, description = "Service name"), LogsQuery), responses((status = 200, description = "Recent log lines; with follow=true a newline-delimited JSON stream of LogEntry", body = Vec<LogEntry>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn logs(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<LogsQuery>,
) -> Result<Response> {
    let machines = logged_machines(&state, &name, |m| {
        matches!(m, MachineState::Running | MachineState::Stopped)
    })
    .await?;
    let tail = query.tail.unwrap_or(DEFAULT_TAIL).min(MAX_TAIL);
    if query.follow {
        return Ok(follow(&state, name, machines, tail));
    }
    if machines.is_empty() {
        return Err(Error::NoMachines(name));
    }
    let mut entries = Vec::new();
    for (machine, container) in machines {
        let lines = state.podman.logs(&container, tail).await?;
        entries.extend(lines.into_iter().map(|line| entry(machine, line)));
    }
    Ok(Json(entries).into_response())
}

// stopped machines keep their containers, so a stopped service still shows its last logs
async fn logged_machines(
    state: &AppState,
    name: &Name,
    wanted: impl Fn(MachineState) -> bool + Send + 'static,
) -> Result<Vec<(MachineId, String)>> {
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
    Ok(machines
        .into_iter()
        .filter(|m| wanted(m.state))
        .filter_map(|m| Some((m.id, m.container_id?)))
        .collect())
}

fn follow(state: &AppState, name: Name, machines: Vec<(MachineId, String)>, tail: u32) -> Response {
    let (lines, receiver) = mpsc::channel(STREAM_BUFFER);
    tokio::spawn(watch(state.clone(), name, machines, tail, lines));
    (
        [(CONTENT_TYPE, "application/x-ndjson")],
        Body::new(ChannelBody(receiver)),
    )
        .into_response()
}

// follows the service, not the machines it had: replacements from a deploy, restart or start join in
async fn watch(
    state: AppState,
    name: Name,
    machines: Vec<(MachineId, String)>,
    tail: u32,
    lines: mpsc::Sender<Bytes>,
) {
    let mut following = JoinSet::new();
    let mut active = HashSet::new();
    let mut ended: HashMap<MachineId, SystemTime> = HashMap::new();
    for (machine, container) in machines {
        active.insert(machine);
        let podman = state.podman.clone();
        following.spawn(forward(
            podman,
            machine,
            container,
            LogStart::Tail(tail),
            lines.clone(),
        ));
    }
    let mut poll = tokio::time::interval(MACHINE_POLL);
    let shutdown = state.shutdown.wait();
    tokio::pin!(shutdown);
    loop {
        tokio::select! {
            () = &mut shutdown => break,
            () = lines.closed() => break,
            Some(done) = following.join_next() => {
                if let Ok(machine) = done {
                    active.remove(&machine);
                    ended.insert(machine, SystemTime::now());
                }
            }
            _ = poll.tick() => {
                let running = match logged_machines(&state, &name, |m| m == MachineState::Running).await {
                    Ok(running) => running,
                    Err(Error::ServiceNotFound(_)) => break,
                    Err(err) => {
                        tracing::debug!(service = %name, error = %err, "could not look for new machines to follow");
                        continue;
                    }
                };
                for (machine, container) in running {
                    if !active.insert(machine) {
                        continue;
                    }
                    // a machine started again continues where its stream ended, a new one from its start
                    let from = ended.get(&machine).map_or(LogStart::Beginning, |at| LogStart::Since(*at));
                    let podman = state.podman.clone();
                    following.spawn(forward(podman, machine, container, from, lines.clone()));
                }
            }
        }
    }
}

async fn forward(
    podman: Podman,
    machine: MachineId,
    container: String,
    from: LogStart,
    lines: mpsc::Sender<Bytes>,
) -> MachineId {
    let mut follower = match podman.follow_logs(&container, from).await {
        Ok(follower) => follower,
        Err(err) => {
            tracing::debug!(%machine, error = %err, "could not follow logs");
            return machine;
        }
    };
    while let Some(next) = follower.next().await {
        let line = match next {
            Ok(line) => line,
            Err(err) => {
                tracing::debug!(%machine, error = %err, "log stream ended");
                break;
            }
        };
        let Ok(mut json) = serde_json::to_vec(&entry(machine, line)) else {
            break;
        };
        json.push(b'\n');
        if lines.send(Bytes::from(json)).await.is_err() {
            break;
        }
    }
    machine
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
