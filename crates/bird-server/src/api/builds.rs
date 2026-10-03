use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use bird_api::{BuildEvent, ErrorBody};
use bird_core::{BuildFile, ImageRef, Name};
use bird_podman::BuildLine;
use bytes::Bytes;
use http_body_util::Limited;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::stream::ChannelBody;
use crate::state::AppState;
use crate::{Error, Result};

const MAX_CONTEXT_BYTES: usize = 512 * 1024 * 1024;
const BUILD_TIMEOUT: Duration = Duration::from_mins(30);
const STREAM_BUFFER: usize = 256;
const DEFAULT_DOCKERFILE: &str = "Dockerfile";

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub(crate) struct BuildQuery {
    /// Dockerfile path inside the context, Dockerfile by default
    #[param(value_type = Option<String>)]
    dockerfile: Option<BuildFile>,
}

/// Build an image from source
#[utoipa::path(post, path = "/v1/services/{name}/builds", tag = "builds", params(("name" = String, Path, description = "Service the image is for"), BuildQuery), request_body(content = Vec<u8>, content_type = "application/x-tar", description = "Build context as a tar archive, optionally gzipped, at most 512 MiB"), responses((status = 200, description = "Newline-delimited JSON stream of BuildEvent, ending with built or failed", body = BuildEvent), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 413, description = "Build context too large", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<BuildQuery>,
    headers: HeaderMap,
    body: Body,
) -> Result<Response> {
    let declared = headers
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok()?.parse::<usize>().ok());
    if declared.is_some_and(|length| length > MAX_CONTEXT_BYTES) {
        return Err(Error::ContextTooLarge(MAX_CONTEXT_BYTES));
    }
    let tag = image_tag(&name)?;
    let dockerfile = query
        .dockerfile
        .map_or_else(|| DEFAULT_DOCKERFILE.to_owned(), |file| file.to_string());
    let (events, receiver) = mpsc::channel(STREAM_BUFFER);
    let context = Limited::new(body, MAX_CONTEXT_BYTES);
    tokio::spawn(run(state, name, tag, dockerfile, context, events));
    Ok((
        [(CONTENT_TYPE, "application/x-ndjson")],
        Body::new(ChannelBody(receiver)),
    )
        .into_response())
}

// a fresh tag per build keeps earlier images around for rollbacks
fn image_tag(name: &Name) -> Result<ImageRef> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    Ok(format!("localhost/bird/{name}:{millis}").parse()?)
}

async fn run(
    state: AppState,
    name: Name,
    tag: ImageRef,
    dockerfile: String,
    context: Limited<Body>,
    events: mpsc::Sender<Bytes>,
) {
    tracing::info!(service = %name, image = %tag, "building");
    let shutdown = state.shutdown.wait();
    let outcome = tokio::select! {
        () = shutdown => Err(Error::BuildFailed("birdd is shutting down".to_owned())),
        timed = tokio::time::timeout(BUILD_TIMEOUT, build(&state, &tag, &dockerfile, context, &events)) => {
            timed.unwrap_or_else(|_| Err(Error::BuildFailed(format!(
                "took longer than {} minutes",
                BUILD_TIMEOUT.as_secs() / 60
            ))))
        }
    };
    let last = match outcome {
        Ok(()) => {
            tracing::info!(service = %name, image = %tag, "built");
            BuildEvent::Built { image: tag }
        }
        Err(err) => {
            tracing::warn!(service = %name, image = %tag, error = %err, "build failed");
            BuildEvent::Failed {
                error: err.to_string(),
            }
        }
    };
    send(&events, &last).await;
}

async fn build(
    state: &AppState,
    tag: &ImageRef,
    dockerfile: &str,
    context: Limited<Body>,
    events: &mpsc::Sender<Bytes>,
) -> Result<()> {
    let mut output = state.podman.build_image(context, tag, dockerfile).await?;
    while let Some(line) = output.next().await {
        match line? {
            BuildLine::Log(line) => {
                // a closed stream means the client left, dropping the build cancels it in podman
                if !send(events, &BuildEvent::Log { line }).await {
                    return Err(Error::BuildFailed("the client disconnected".to_owned()));
                }
            }
            BuildLine::Failed(error) => return Err(Error::BuildFailed(error)),
        }
    }
    Ok(())
}

async fn send(events: &mpsc::Sender<Bytes>, event: &BuildEvent) -> bool {
    let Ok(mut json) = serde_json::to_vec(event) else {
        return false;
    };
    json.push(b'\n');
    events.send(Bytes::from(json)).await.is_ok()
}
