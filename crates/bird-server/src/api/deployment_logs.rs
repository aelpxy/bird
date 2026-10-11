use axum::Json;
use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{IntoResponse, Response};
use bird_api::{DeployLogLine, DeploymentLog, ErrorBody};
use bird_core::DeploymentId;
use bytes::Bytes;
use serde::Deserialize;
use tokio::sync::{broadcast, mpsc};

use super::scope::ServiceScope;
use super::stream::ChannelBody;
use crate::deploy::running;
use crate::state::AppState;
use crate::{Error, Result};

const STREAM_BUFFER: usize = 256;

#[derive(Debug, Deserialize, utoipa::IntoParams)]
pub(crate) struct LogQuery {
    /// Keep the response open and send new lines while the deploy runs
    #[serde(default)]
    follow: bool,
}

#[derive(Deserialize)]
pub(crate) struct DeploymentPath {
    id: DeploymentId,
}

/// What a deploy did: build output, birdd's steps, the release command and new machines' output
#[utoipa::path(get, operation_id = "get_deployment_log", path = "/v1/projects/{project}/environments/{environment}/services/{name}/deployments/{id}/log", tag = "deployments", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ("id" = String, Path, description = "Deployment id"), LogQuery), responses((status = 200, description = "The log; with follow=true a newline-delimited JSON stream of DeployLogLine that ends with the deploy", body = DeploymentLog), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or deployment not found", body = ErrorBody)))]
pub(crate) async fn get(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Path(DeploymentPath { id }): Path<DeploymentPath>,
    Query(query): Query<LogQuery>,
) -> Result<Response> {
    let service = state.service(environment, &name).await?;
    let deployment = state.db.call(move |store| store.deployment(id)).await?;
    if deployment.is_none_or(|deployment| deployment.service_id != service.id) {
        return Err(Error::Store(bird_store::Error::NotFound("deployment")));
    }
    let live = running(&state).get(&id).cloned();
    let (lines, updates) = match live {
        Some(log) => log.follow(),
        None => (saved(&state, id).await?, None),
    };
    if !query.follow {
        let done = updates.is_none();
        return Ok(Json(DeploymentLog { lines, done }).into_response());
    }
    let (events, receiver) = mpsc::channel(STREAM_BUFFER);
    tokio::spawn(forward(lines, updates, events));
    Ok((
        [(CONTENT_TYPE, "application/x-ndjson")],
        Body::new(ChannelBody(receiver)),
    )
        .into_response())
}

async fn saved(state: &AppState, id: DeploymentId) -> Result<Vec<DeployLogLine>> {
    let Some(encoded) = state.db.call(move |store| store.deployment_log(id)).await? else {
        return Ok(Vec::new());
    };
    serde_json::from_str(&encoded).map_err(|err| std::io::Error::other(err).into())
}

// sent until the deploy ends, which drops the last sender and closes the channel
async fn forward(
    lines: Vec<DeployLogLine>,
    updates: Option<broadcast::Receiver<DeployLogLine>>,
    events: mpsc::Sender<Bytes>,
) {
    for line in &lines {
        if !send(&events, line).await {
            return;
        }
    }
    let Some(mut updates) = updates else {
        return;
    };
    loop {
        match updates.recv().await {
            Ok(next) => {
                if !send(&events, &next).await {
                    return;
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => {}
            Err(broadcast::error::RecvError::Closed) => return,
        }
    }
}

async fn send(events: &mpsc::Sender<Bytes>, line: &DeployLogLine) -> bool {
    let Ok(mut json) = serde_json::to_vec(line) else {
        return false;
    };
    json.push(b'\n');
    events.send(Bytes::from(json)).await.is_ok()
}
