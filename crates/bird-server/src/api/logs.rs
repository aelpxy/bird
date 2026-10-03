use axum::Json;
use axum::extract::{Path, Query, State};
use bird_api::{LogEntry, LogStream};
use bird_core::{MachineState, Name};
use serde::Deserialize;

use crate::state::AppState;
use crate::{Error, Result};

const DEFAULT_TAIL: u32 = 100;
const MAX_TAIL: u32 = 10_000;

#[derive(Deserialize)]
pub(crate) struct TailQuery {
    tail: Option<u32>,
}

pub(crate) async fn tail(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<TailQuery>,
) -> Result<Json<Vec<LogEntry>>> {
    let environment_id = state.environment_id;
    let lookup = name.clone();
    let container_id = state
        .db
        .call(move |store| {
            let Some(service) = store.service_by_name(environment_id, &lookup)? else {
                return Ok(None);
            };
            let Some(deployment) = store.active_deployment(service.id)? else {
                return Ok(Some(None));
            };
            let container = store
                .list_machines(deployment.id)?
                .into_iter()
                .filter(|m| m.state == MachineState::Running)
                .find_map(|m| m.container_id);
            Ok(Some(container))
        })
        .await?
        .ok_or_else(|| Error::ServiceNotFound(name.clone()))?
        .ok_or(Error::NoMachines(name))?;

    let lines = query.tail.unwrap_or(DEFAULT_TAIL).min(MAX_TAIL);
    let entries = state
        .podman
        .logs(&container_id, lines)
        .await?
        .into_iter()
        .map(|line| LogEntry {
            stream: match line.stream {
                bird_podman::LogStream::Stdout => LogStream::Stdout,
                bird_podman::LogStream::Stderr => LogStream::Stderr,
            },
            text: line.text,
        })
        .collect();
    Ok(Json(entries))
}
