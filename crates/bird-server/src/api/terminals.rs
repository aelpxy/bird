use std::sync::Arc;

use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::http::header::{CONNECTION, UPGRADE};
use axum::response::{IntoResponse, Response};
use bird_api::{ErrorBody, TTY_UPGRADE};
use bird_core::{Command, Name};
use hyper::upgrade::OnUpgrade;
use serde::Deserialize;
use tokio::sync::OwnedSemaphorePermit;

use crate::commands::{
    self,
    tty::{Remote, Stdio},
};
use crate::state::AppState;
use crate::{Error, Result};

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ExecQuery {
    /// The command as a JSON array of strings, like `["psql","-U","postgres"]`
    command: String,
    /// Machine id, or the end of one; left out, the first running machine
    machine: Option<String>,
    /// Terminal width in columns
    cols: u16,
    /// Terminal height in rows
    rows: u16,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct RunQuery {
    /// The command as a JSON array of strings, like `["rails","console"]`
    command: String,
    /// Terminal width in columns
    cols: u16,
    /// Terminal height in rows
    rows: u16,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PipedExecQuery {
    /// The command as a JSON array of strings, like `["pg_dump","-Fc","app"]`
    command: String,
    /// Machine id, or the end of one; left out, the first running machine
    machine: Option<String>,
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct PipedRunQuery {
    /// The command as a JSON array of strings, like `["rake","db:migrate"]`
    command: String,
}

/// Open an interactive terminal in a running machine
#[utoipa::path(get, path = "/v1/services/{name}/exec/tty", tag = "commands", params(("name" = String, Path, description = "Service name"), ExecQuery), responses((status = 101, description = "Switched to bird-tty: both sides send frames of a kind byte, a big-endian u32 length and the payload; data (0) both ways, resize (1, cols and rows as u16) from the client, then exit (2, i32 code) or error (3, text) from birdd"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn exec(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<ExecQuery>,
    request: Request,
) -> Result<Response> {
    let (upgrade, command, permit) = accept(&state, request, &query.command)?;
    let container = commands::pick_machine(&state, &name, query.machine.as_deref()).await?;
    let session = state.podman.exec_tty(&container, command.args()).await?;
    if let Err(err) = state
        .podman
        .resize_exec(session.id(), query.cols, query.rows)
        .await
    {
        tracing::debug!(error = %err, "could not size the new terminal");
    }
    // the command itself is not logged, its arguments may hold secrets
    tracing::info!(service = %name, "terminal opened");
    let remote = Remote::Exec(session.id().to_owned());
    let stdio = Stdio::Terminal {
        cols: query.cols,
        rows: query.rows,
    };
    tokio::spawn(commands::tty::serve(
        state, name, upgrade, session.io, remote, stdio, permit,
    ));
    Ok(switched())
}

/// Run a command in a running machine with its stdin, stdout and stderr passed through as bytes
#[utoipa::path(get, path = "/v1/services/{name}/exec/pipe", tag = "commands", params(("name" = String, Path, description = "Service name"), PipedExecQuery), responses((status = 101, description = "Switched to bird-tty without a terminal: the client sends data (0, stdin) and eof (5, empty) once its stdin ends; birdd sends data (0, stdout) and stderr (4) as written, then exit (2) or error (3)"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn exec_pipe(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<PipedExecQuery>,
    request: Request,
) -> Result<Response> {
    let (upgrade, command, permit) = accept(&state, request, &query.command)?;
    let container = commands::pick_machine(&state, &name, query.machine.as_deref()).await?;
    let session = state.podman.exec_piped(&container, command.args()).await?;
    tracing::info!(service = %name, "piped command started");
    let remote = Remote::Exec(session.id().to_owned());
    tokio::spawn(commands::tty::serve(
        state,
        name,
        upgrade,
        session.io,
        remote,
        Stdio::Piped,
        permit,
    ));
    Ok(switched())
}

/// Open an interactive terminal in a new container from the service's image
#[utoipa::path(get, path = "/v1/services/{name}/run/tty", tag = "commands", params(("name" = String, Path, description = "Service name"), RunQuery), responses((status = 101, description = "Switched to bird-tty, like the exec terminal; the container is removed when the session ends"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn run(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<RunQuery>,
    request: Request,
) -> Result<Response> {
    let (upgrade, command, permit) = accept(&state, request, &query.command)?;
    let target = commands::run_target(&state, &name).await?;
    let stdio = Stdio::Terminal {
        cols: query.cols,
        rows: query.rows,
    };
    let (id, io) = commands::start_attached(&state, &target, &command, stdio).await?;
    tokio::spawn(commands::tty::serve(
        state,
        name,
        upgrade,
        io,
        Remote::Container(id),
        stdio,
        permit,
    ));
    Ok(switched())
}

/// Run a command in a new container from the service's image with its stdio passed through as bytes
#[utoipa::path(get, path = "/v1/services/{name}/run/pipe", tag = "commands", params(("name" = String, Path, description = "Service name"), PipedRunQuery), responses((status = 101, description = "Switched to bird-tty without a terminal, like the piped exec; the container is removed when the session ends"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn run_pipe(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<PipedRunQuery>,
    request: Request,
) -> Result<Response> {
    let (upgrade, command, permit) = accept(&state, request, &query.command)?;
    let target = commands::run_target(&state, &name).await?;
    let (id, io) = commands::start_attached(&state, &target, &command, Stdio::Piped).await?;
    tokio::spawn(commands::tty::serve(
        state,
        name,
        upgrade,
        io,
        Remote::Container(id),
        Stdio::Piped,
        permit,
    ));
    Ok(switched())
}

// checked before anything starts, so a refused request leaves no process or container behind
fn accept(
    state: &AppState,
    mut request: Request,
    command: &str,
) -> Result<(OnUpgrade, Command, OwnedSemaphorePermit)> {
    let wants_tty = request.headers().get(UPGRADE).is_some_and(|value| {
        value
            .as_bytes()
            .eq_ignore_ascii_case(TTY_UPGRADE.as_bytes())
    });
    let Some(upgrade) = request
        .extensions_mut()
        .remove::<OnUpgrade>()
        .filter(|_| wants_tty)
    else {
        return Err(Error::UpgradeRequired);
    };
    let args: Vec<String> = serde_json::from_str(command)
        .map_err(|err| Error::InvalidTtyRequest(format!("command: {err}")))?;
    let command = Command::try_from(args)?;
    let permit = Arc::clone(&state.terminals)
        .try_acquire_owned()
        .map_err(|_| Error::TooManyTerminals)?;
    Ok((upgrade, command, permit))
}

fn switched() -> Response {
    (
        StatusCode::SWITCHING_PROTOCOLS,
        [(CONNECTION, "upgrade"), (UPGRADE, TTY_UPGRADE)],
    )
        .into_response()
}
