use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::{DeploymentSummary, MachineSummary, ServiceSummary};
use bird_core::{MachineState, Name};
use bird_store::Store;

use crate::state::AppState;
use crate::{Result, deploy};

pub(crate) async fn list(State(state): State<AppState>) -> Result<Json<Vec<ServiceSummary>>> {
    let environment_id = state.environment_id;
    let summaries = state
        .db
        .call(move |store| {
            store
                .list_services(environment_id)?
                .into_iter()
                .map(|service| summarize(store, service))
                .collect::<bird_store::Result<Vec<_>>>()
        })
        .await?;
    Ok(Json(summaries))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<StatusCode> {
    let task = tokio::spawn(async move { deploy::remove_service(&state, name).await });
    match task.await {
        Ok(result) => result.map(|()| StatusCode::NO_CONTENT),
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}

fn summarize(store: &Store, service: bird_core::Service) -> bird_store::Result<ServiceSummary> {
    let domains = store
        .list_domains(service.id)?
        .into_iter()
        .map(|d| d.hostname)
        .collect();
    let Some(active) = store.active_deployment(service.id)? else {
        return Ok(ServiceSummary {
            name: service.name,
            image: service.image,
            port: service.port,
            replicas: service.replicas,
            domains,
            deployment: None,
        });
    };
    let machines = store
        .list_machines(active.id)?
        .into_iter()
        .filter(|m| m.state != MachineState::Destroyed)
        .map(|m| MachineSummary {
            id: m.id,
            state: m.state,
        })
        .collect();
    Ok(ServiceSummary {
        name: service.name,
        image: active.image,
        port: active.port,
        replicas: service.replicas,
        domains,
        deployment: Some(DeploymentSummary {
            id: active.id,
            status: active.status,
            machines,
        }),
    })
}
