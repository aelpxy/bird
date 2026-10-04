use std::future::Future;
use std::sync::Arc;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::StatusCode;
use axum::http::header::{CONNECTION, CONTENT_TYPE, UPGRADE};
use axum::response::{IntoResponse, Response};
use bird_api::{CommandEvent, ErrorBody, ExecRequest, RunRequest, TTY_UPGRADE};
use bird_core::{Command, Name};
use bytes::Bytes;
use hyper::upgrade::OnUpgrade;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::stream::ChannelBody;
use crate::state::AppState;
use crate::{Error, Result, commands};

const STREAM_BUFFER: usize = 256;

/// Run a command in a running machine
#[utoipa::path(post, path = "/v1/services/{name}/exec", tag = "commands", params(("name" = String, Path, description = "Service name")), request_body = ExecRequest, responses((status = 200, description = "Newline-delimited JSON stream of CommandEvent, ending with exited or failed", body = CommandEvent), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn exec(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(request): Json<ExecRequest>,
) -> Result<Response> {
    let container = commands::pick_machine(&state, &name, request.machine.as_deref()).await?;
    // the command itself is not logged, its arguments may hold secrets
    tracing::info!(service = %name, "exec in machine");
    Ok(stream(move |events| async move {
        commands::exec(&state, &container, &request.command, &events).await
    }))
}

/// Run a command in a new container from the service's image
#[utoipa::path(post, path = "/v1/services/{name}/run", tag = "commands", params(("name" = String, Path, description = "Service name")), request_body = RunRequest, responses((status = 200, description = "Newline-delimited JSON stream of CommandEvent, ending with exited or failed", body = CommandEvent), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn run(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(request): Json<RunRequest>,
) -> Result<Response> {
    let target = commands::run_target(&state, &name).await?;
    Ok(stream(move |events| async move {
        commands::run(&state, &target, &request.command, &events).await
    }))
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct TtyQuery {
    /// The command as a JSON array of strings, like `["psql","-U","postgres"]`
    command: String,
    /// Machine id, or the end of one; left out, the first running machine
    machine: Option<String>,
    /// Terminal width in columns
    cols: u16,
    /// Terminal height in rows
    rows: u16,
}

/// Open an interactive terminal in a running machine
#[utoipa::path(get, path = "/v1/services/{name}/exec/tty", tag = "commands", params(("name" = String, Path, description = "Service name"), TtyQuery), responses((status = 101, description = "Switched to bird-tty: both sides send frames of a kind byte, a big-endian u32 length and the payload; data (0) both ways, resize (1, cols and rows as u16) from the client, then exit (2, i32 code) or error (3, text) from birdd"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tty", body = ErrorBody), (status = 503, description = "Too many terminals are open", body = ErrorBody)))]
pub(crate) async fn tty(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<TtyQuery>,
    mut request: Request,
) -> Result<Response> {
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
    let args: Vec<String> = serde_json::from_str(&query.command)
        .map_err(|err| Error::InvalidTtyRequest(format!("command: {err}")))?;
    let command = Command::try_from(args)?;
    let permit = Arc::clone(&state.terminals)
        .try_acquire_owned()
        .map_err(|_| Error::TooManyTerminals)?;
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
    tokio::spawn(commands::tty::serve(state, name, upgrade, session, permit));
    Ok((
        StatusCode::SWITCHING_PROTOCOLS,
        [(CONNECTION, "upgrade"), (UPGRADE, TTY_UPGRADE)],
    )
        .into_response())
}

// errors before the command starts are plain http errors; after that they end the stream
fn stream<F, Fut>(work: F) -> Response
where
    F: FnOnce(mpsc::Sender<Bytes>) -> Fut + Send + 'static,
    Fut: Future<Output = Result<Option<i32>>> + Send + 'static,
{
    let (events, receiver) = mpsc::channel(STREAM_BUFFER);
    tokio::spawn(async move {
        let last = match work(events.clone()).await {
            Ok(Some(code)) => CommandEvent::Exited { code },
            Ok(None) => return,
            Err(err) => CommandEvent::Failed {
                error: err.to_string(),
            },
        };
        commands::send(&events, &last).await;
    });
    (
        [(CONTENT_TYPE, "application/x-ndjson")],
        Body::new(ChannelBody(receiver)),
    )
        .into_response()
}
