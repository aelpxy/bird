use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use bird_api::ErrorBody;
use bird_api::{DeploymentSummary, MachineSummary, ServiceSummary, VolumeSpec};
use bird_core::{MachineState, Name};
use bird_store::Store;
use serde::Deserialize;

use crate::state::AppState;
use crate::{Result, deploy};

/// List services
#[utoipa::path(get, path = "/v1/services", tag = "services", responses((status = 200, description = "Services in the default environment", body = Vec<ServiceSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
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

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct RemoveQuery {
    /// Also delete the service's volumes and the data on them; required when it has any
    #[serde(default)]
    purge: bool,
}

/// Remove a service and its machines
#[utoipa::path(delete, path = "/v1/services/{name}", tag = "services", params(("name" = String, Path, description = "Service name"), RemoveQuery), responses((status = 204, description = "Service and its machines removed"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Query(query): Query<RemoveQuery>,
) -> Result<StatusCode> {
    let task = tokio::spawn(async move { deploy::remove_service(&state, name, query.purge).await });
    match task.await {
        Ok(result) => result.map(|()| StatusCode::NO_CONTENT),
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}

fn summarize(store: &Store, service: bird_core::Service) -> bird_store::Result<ServiceSummary> {
    let volumes = store
        .list_volumes(service.id)?
        .into_iter()
        .map(|v| VolumeSpec {
            name: v.name,
            path: v.mount_path,
        })
        .collect();
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
            memory: service.memory,
            cpus: service.cpus,
            health: service.health,
            health_timeout: service.health_timeout,
            domains,
            volumes,
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
        memory: service.memory,
        cpus: service.cpus,
        health: service.health,
        health_timeout: service.health_timeout,
        domains,
        volumes,
        deployment: Some(DeploymentSummary {
            id: active.id,
            status: active.status,
            machines,
        }),
    })
}
