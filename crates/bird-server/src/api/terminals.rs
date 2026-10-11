use std::future::Future;
use std::sync::Arc;

use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{FromRequestParts, Query, Request, State};
use axum::http::header::{CONNECTION, UPGRADE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bird_api::{ErrorBody, MAX_FRAME_BYTES, TTY_UPGRADE};
use bird_core::{Command, Name};
use hyper::upgrade::{OnUpgrade, Upgraded};
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use tokio::sync::OwnedSemaphorePermit;

use super::scope::ServiceScope;
use crate::commands::{
    self,
    tty::{Remote, Stdio},
};
use crate::state::AppState;
use crate::{Error, Result};

const WEBSOCKET_UPGRADE: &[u8] = b"websocket";

// the cli speaks bird-tty on the raw connection; browsers cannot, so they wrap it in a websocket
enum Client {
    Tty(OnUpgrade),
    WebSocket(WebSocketUpgrade),
}

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
    /// Run the command itself instead of passing it to the image's entrypoint
    #[serde(default)]
    skip_entrypoint: bool,
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
    /// Run the command itself instead of passing it to the image's entrypoint
    #[serde(default)]
    skip_entrypoint: bool,
}

/// Open an interactive terminal in a running machine
#[utoipa::path(get, operation_id = "open_exec_terminal", path = "/v1/projects/{project}/environments/{environment}/services/{name}/exec/tty", tag = "commands", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ExecQuery), responses((status = 101, description = "Switched to bird-tty: both sides send frames of a kind byte, a big-endian u32 length and the payload; data (0) both ways, resize (1, cols and rows as u16) from the client, then exit (2, i32 code) or error (3, text) from birdd. Browsers can upgrade to websocket instead, which carries the same frames in binary messages"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty or websocket", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn exec(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<ExecQuery>,
    request: Request,
) -> Result<Response> {
    let (client, command, permit) = accept(&state, request, &query.command).await?;
    let container =
        commands::pick_machine(&state, environment, &name, query.machine.as_deref()).await?;
    let start = {
        let (state, name) = (state.clone(), name.clone());
        async move {
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
            Ok((session.io, remote))
        }
    };
    let stdio = Stdio::Terminal {
        cols: query.cols,
        rows: query.rows,
    };
    open(state, name, client, start, stdio, permit).await
}

/// Run a command in a running machine with its stdin, stdout and stderr passed through as bytes
#[utoipa::path(get, path = "/v1/projects/{project}/environments/{environment}/services/{name}/exec/pipe", tag = "commands", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), PipedExecQuery), responses((status = 101, description = "Switched to bird-tty without a terminal: the client sends data (0, stdin) and eof (5, empty) once its stdin ends; birdd sends data (0, stdout) and stderr (4) as written, then exit (2) or error (3)"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty or websocket", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn exec_pipe(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<PipedExecQuery>,
    request: Request,
) -> Result<Response> {
    let (client, command, permit) = accept(&state, request, &query.command).await?;
    let container =
        commands::pick_machine(&state, environment, &name, query.machine.as_deref()).await?;
    let start = {
        let (state, name) = (state.clone(), name.clone());
        async move {
            let session = state.podman.exec_piped(&container, command.args()).await?;
            tracing::info!(service = %name, "piped command started");
            let remote = Remote::Exec(session.id().to_owned());
            Ok((session.io, remote))
        }
    };
    open(state, name, client, start, Stdio::Piped, permit).await
}

/// Open an interactive terminal in a new container from the service's image
#[utoipa::path(get, operation_id = "open_run_terminal", path = "/v1/projects/{project}/environments/{environment}/services/{name}/run/tty", tag = "commands", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), RunQuery), responses((status = 101, description = "Switched to bird-tty, like the exec terminal; the container is removed when the session ends"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty or websocket", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn run(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<RunQuery>,
    request: Request,
) -> Result<Response> {
    let (client, command, permit) = accept(&state, request, &query.command).await?;
    let target = commands::run_target(&state, environment, &name)
        .await?
        .skipping_entrypoint(query.skip_entrypoint);
    let stdio = Stdio::Terminal {
        cols: query.cols,
        rows: query.rows,
    };
    let start = attached(state.clone(), target, command, stdio);
    open(state, name, client, start, stdio, permit).await
}

/// Run a command in a new container from the service's image with its stdio passed through as bytes
#[utoipa::path(get, path = "/v1/projects/{project}/environments/{environment}/services/{name}/run/pipe", tag = "commands", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), PipedRunQuery), responses((status = 101, description = "Switched to bird-tty without a terminal, like the piped exec; the container is removed when the session ends"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty or websocket", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn run_pipe(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<PipedRunQuery>,
    request: Request,
) -> Result<Response> {
    let (client, command, permit) = accept(&state, request, &query.command).await?;
    let target = commands::run_target(&state, environment, &name)
        .await?
        .skipping_entrypoint(query.skip_entrypoint);
    let start = attached(state.clone(), target, command, Stdio::Piped);
    open(state, name, client, start, Stdio::Piped, permit).await
}

async fn attached(
    state: AppState,
    target: commands::RunTarget,
    command: Command,
    stdio: Stdio,
) -> Result<(TokioIo<Upgraded>, Remote)> {
    let (id, io) = commands::start_attached(&state, &target, &command, stdio).await?;
    Ok((io, Remote::Container(id)))
}

// a bird-tty client gets errors as http statuses; a websocket's command starts once it is open
async fn open(
    state: AppState,
    name: Name,
    client: Client,
    start: impl Future<Output = Result<(TokioIo<Upgraded>, Remote)>> + Send + 'static,
    stdio: Stdio,
    permit: OwnedSemaphorePermit,
) -> Result<Response> {
    match client {
        Client::Tty(upgrade) => {
            let (io, remote) = start.await?;
            tokio::spawn(commands::tty::serve(
                state, name, upgrade, io, remote, stdio, permit,
            ));
            Ok(switched())
        }
        Client::WebSocket(socket) => Ok(socket.on_upgrade(move |socket| {
            commands::websocket::serve(state, name, socket, start, stdio, permit)
        })),
    }
}

// checked before anything starts, so a refused request leaves no process or container behind
async fn accept(
    state: &AppState,
    request: Request,
    command: &str,
) -> Result<(Client, Command, OwnedSemaphorePermit)> {
    let (mut parts, _) = request.into_parts();
    let asked = parts.headers.get(UPGRADE).map(HeaderValue::as_bytes);
    let client = match asked {
        Some(protocol) if protocol.eq_ignore_ascii_case(WEBSOCKET_UPGRADE) => {
            let socket = WebSocketUpgrade::from_request_parts(&mut parts, &())
                .await
                .map_err(|rejection| Error::InvalidTtyRequest(rejection.body_text()))?;
            Client::WebSocket(socket.max_message_size(MAX_FRAME_BYTES))
        }
        Some(protocol) if protocol.eq_ignore_ascii_case(TTY_UPGRADE.as_bytes()) => parts
            .extensions
            .remove::<OnUpgrade>()
            .map(Client::Tty)
            .ok_or(Error::UpgradeRequired)?,
        _ => return Err(Error::UpgradeRequired),
    };
    let args: Vec<String> = serde_json::from_str(command)
        .map_err(|err| Error::InvalidTtyRequest(format!("command: {err}")))?;
    let command = Command::try_from(args)?;
    let permit = Arc::clone(&state.terminals)
        .try_acquire_owned()
        .map_err(|_| Error::TooManyTerminals)?;
    Ok((client, command, permit))
}

fn switched() -> Response {
    (
        StatusCode::SWITCHING_PROTOCOLS,
        [(CONNECTION, "upgrade"), (UPGRADE, TTY_UPGRADE)],
    )
        .into_response()
}
