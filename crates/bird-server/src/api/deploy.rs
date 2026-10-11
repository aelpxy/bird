use axum::Json;
use axum::body::Body;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::http::header::{ACCEPT, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use bird_api::{DeployEvent, DeployLogLine, DeployRequest, DeployResponse, ErrorBody};
use bytes::Bytes;
use tokio::sync::{broadcast, mpsc};
use tokio::task::JoinHandle;

use super::scope::Scope;
use super::stream::ChannelBody;
use crate::deploy::DeployLog;
use crate::state::AppState;
use crate::{Result, deploy};

const NDJSON: &str = "application/x-ndjson";
const STREAM_BUFFER: usize = 256;

/// Deploy an image, creating the service on first deploy
#[utoipa::path(post, operation_id = "deploy", path = "/v1/projects/{project}/environments/{environment}/deploy", tag = "deployments", request_body = DeployRequest, params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name")), responses((status = 200, description = "Deployed and serving traffic; with `Accept: application/x-ndjson` a newline-delimited JSON stream of DeployEvent instead, ending with deployed or failed", body = DeployResponse), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody), (status = 502, description = "The app did not become healthy; includes its last log lines", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Scope(environment): Scope,
    headers: HeaderMap,
    Json(request): Json<DeployRequest>,
) -> Response {
    let log = DeployLog::new();
    let live = log.subscribe();
    // run detached so a disconnecting client cannot abort a deploy halfway through
    let task =
        tokio::spawn(async move { deploy::deploy(&state, environment, request, &log).await });
    match live.filter(|_| wants_stream(&headers)) {
        Some(live) => stream(live, task),
        None => match outcome(task).await {
            Ok(response) => Json(response).into_response(),
            Err(err) => err.into_response(),
        },
    }
}

fn wants_stream(headers: &HeaderMap) -> bool {
    headers
        .get(ACCEPT)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|accept| accept.contains(NDJSON))
}

async fn outcome(task: JoinHandle<Result<DeployResponse>>) -> Result<DeployResponse> {
    task.await
        .unwrap_or_else(|err| Err(std::io::Error::other(err).into()))
}

fn stream(
    updates: broadcast::Receiver<DeployLogLine>,
    task: JoinHandle<Result<DeployResponse>>,
) -> Response {
    let (events, receiver) = mpsc::channel(STREAM_BUFFER);
    tokio::spawn(forward(updates, task, events));
    ([(CONTENT_TYPE, NDJSON)], Body::new(ChannelBody(receiver))).into_response()
}

// a client that leaves stops the forwarding, never the deploy
async fn forward(
    mut updates: broadcast::Receiver<DeployLogLine>,
    task: JoinHandle<Result<DeployResponse>>,
    events: mpsc::Sender<Bytes>,
) {
    let mut task = std::pin::pin!(outcome(task));
    let result = loop {
        tokio::select! {
            biased;
            line = updates.recv() => match line {
                Ok(next) => {
                    if !send(&events, &DeployEvent::Log(next)).await {
                        return;
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {}
                Err(broadcast::error::RecvError::Closed) => break task.await,
            },
            result = &mut task => break result,
        }
    };
    while let Ok(line) = updates.try_recv() {
        if !send(&events, &DeployEvent::Log(line)).await {
            return;
        }
    }
    let last = match result {
        Ok(response) => DeployEvent::Deployed(response),
        Err(err) => DeployEvent::Failed(super::error::body(&err)),
    };
    send(&events, &last).await;
}

async fn send(events: &mpsc::Sender<Bytes>, event: &DeployEvent) -> bool {
    let Ok(mut json) = serde_json::to_vec(event) else {
        return false;
    };
    json.push(b'\n');
    events.send(Bytes::from(json)).await.is_ok()
}
