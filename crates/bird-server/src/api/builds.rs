use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::http::header::{CONTENT_LENGTH, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use bird_api::{BuildEvent, ErrorBody};
use bird_core::{BuildFile, EnvKey, ImageRef, Name};
use bird_podman::BuildLine;
use bytes::Bytes;
use http_body_util::Limited;
use serde::Deserialize;
use tokio::sync::mpsc;

use super::scope::ServiceScope;
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
    /// Dockerfile ARG values as a JSON object of strings
    args: Option<String>,
}

/// Build an image from source
#[utoipa::path(post, path = "/v1/projects/{project}/environments/{environment}/services/{name}/builds", tag = "builds", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service the image is for"), BuildQuery), request_body(content = Vec<u8>, content_type = "application/x-tar", description = "Build context as a tar archive, optionally gzipped, at most 512 MiB"), responses((status = 200, description = "Newline-delimited JSON stream of BuildEvent, ending with built or failed", body = BuildEvent), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 413, description = "Build context too large", body = ErrorBody)))]
pub(crate) async fn create(
    State(state): State<AppState>,
    // built images are named by service only; rollbacks and cleanup go by deployment records
    ServiceScope { name, .. }: ServiceScope,
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
    let args: BTreeMap<EnvKey, String> = match &query.args {
        Some(json) => {
            serde_json::from_str(json).map_err(|err| Error::InvalidBuildArgs(err.to_string()))?
        }
        None => BTreeMap::new(),
    };
    let tag = image_tag(&name)?;
    let dockerfile = query
        .dockerfile
        .map_or_else(|| DEFAULT_DOCKERFILE.to_owned(), |file| file.to_string());
    let (events, receiver) = mpsc::channel(STREAM_BUFFER);
    let context = Limited::new(body, MAX_CONTEXT_BYTES);
    let build = Build {
        tag,
        dockerfile,
        args,
    };
    tokio::spawn(run(state, name, build, context, events));
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

struct Build {
    tag: ImageRef,
    dockerfile: String,
    args: BTreeMap<EnvKey, String>,
}

async fn run(
    state: AppState,
    name: Name,
    build: Build,
    context: Limited<Body>,
    events: mpsc::Sender<Bytes>,
) {
    let shutdown = state.shutdown.wait();
    let outcome = tokio::select! {
        () = shutdown => Err(Error::BuildFailed("birdd is shutting down".to_owned())),
        () = events.closed() => Err(Error::BuildFailed("the client disconnected".to_owned())),
        result = queue_and_build(&state, &name, &build, context, &events) => result,
    };
    let last = match outcome {
        Ok(()) => {
            tracing::info!(service = %name, image = %build.tag, "built");
            BuildEvent::Built { image: build.tag }
        }
        Err(err) => {
            tracing::warn!(service = %name, image = %build.tag, error = %err, "build failed");
            BuildEvent::Failed {
                error: err.to_string(),
            }
        }
    };
    send(&events, &last).await;
}

async fn queue_and_build(
    state: &AppState,
    name: &Name,
    build: &Build,
    context: Limited<Body>,
    events: &mpsc::Sender<Bytes>,
) -> Result<()> {
    let permit = if let Ok(permit) = Arc::clone(&state.builds).try_acquire_owned() {
        permit
    } else {
        let waiting = "waiting for another build to finish...".to_owned();
        send(events, &BuildEvent::Log { line: waiting }).await;
        Arc::clone(&state.builds)
            .acquire_owned()
            .await
            .map_err(|_| Error::BuildFailed("birdd is shutting down".to_owned()))?
    };
    tracing::info!(service = %name, image = %build.tag, "building");
    let outcome = tokio::time::timeout(BUILD_TIMEOUT, run_build(state, build, context, events))
        .await
        .unwrap_or_else(|_| {
            Err(Error::BuildFailed(format!(
                "took longer than {} minutes",
                BUILD_TIMEOUT.as_secs() / 60
            )))
        });
    drop(permit);
    outcome
}

async fn run_build(
    state: &AppState,
    build: &Build,
    context: Limited<Body>,
    events: &mpsc::Sender<Bytes>,
) -> Result<()> {
    let mut output = state
        .podman
        .build_image(context, &build.tag, &build.dockerfile, &build.args)
        .await?;
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
