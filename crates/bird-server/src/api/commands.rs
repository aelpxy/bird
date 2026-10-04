use std::future::Future;

use axum::Json;
use axum::body::Body;
use axum::extract::{Path, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use bird_api::{CommandEvent, ErrorBody, ExecRequest, RunRequest};
use bird_core::Name;
use bytes::Bytes;
use tokio::sync::mpsc;

use super::stream::ChannelBody;
use crate::state::AppState;
use crate::{Result, commands};

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
