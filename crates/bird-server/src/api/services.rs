use axum::Json;
use axum::extract::{Query, State};
use axum::http::StatusCode;
use bird_api::ErrorBody;
use bird_api::{DeploymentSummary, FailedDeploy, MachineSummary, ServiceSummary, VolumeSpec};
use bird_core::{DeploymentStatus, EnvironmentId, MachineId, MachineState, Name};
use bird_store::Store;
use serde::Deserialize;

use super::scope::{Scope, ServiceScope};
use crate::state::AppState;
use crate::{Result, deploy, stats};

/// List services
#[utoipa::path(get, path = "/v1/projects/{project}/environments/{environment}/services", tag = "services", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name")), responses((status = 200, description = "Services in the default environment", body = Vec<ServiceSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Scope(environment_id): Scope,
) -> Result<Json<Vec<ServiceSummary>>> {
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
pub(crate) struct GetQuery {
    /// Also sample cpu, memory and network for running machines, which takes about half a second
    #[serde(default)]
    stats: bool,
}

/// Show one service
#[utoipa::path(get, path = "/v1/projects/{project}/environments/{environment}/services/{name}", tag = "services", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), GetQuery), responses((status = 200, description = "The service, its active deployment and machines", body = ServiceSummary), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn get(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<GetQuery>,
) -> Result<Json<ServiceSummary>> {
    let mut summary = summary(&state, environment, &name).await?;
    if query.stats {
        add_stats(&state, &mut summary).await?;
    }
    Ok(Json(summary))
}

async fn add_stats(state: &AppState, summary: &mut ServiceSummary) -> Result<()> {
    let Some(deployment) = &mut summary.deployment else {
        return Ok(());
    };
    let deployment_id = deployment.id;
    let containers: Vec<(MachineId, String)> = state
        .db
        .call(move |store| store.list_machines(deployment_id))
        .await?
        .into_iter()
        .filter(|m| m.state == MachineState::Running)
        .filter_map(|m| Some((m.id, m.container_id?)))
        .collect();
    let mut sampled = stats::sample(state, &containers).await;
    for machine in &mut deployment.machines {
        machine.stats = sampled.remove(&machine.id);
    }
    Ok(())
}

pub(super) async fn summary(
    state: &AppState,
    environment: EnvironmentId,
    name: &Name,
) -> Result<ServiceSummary> {
    let service = state.service(environment, name).await?;
    state.db.call(move |store| summarize(store, service)).await
}

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct RemoveQuery {
    /// Also delete the service's volumes and the data on them; required when it has any
    #[serde(default)]
    purge: bool,
}

/// Remove a service and its machines
#[utoipa::path(delete, path = "/v1/projects/{project}/environments/{environment}/services/{name}", tag = "services", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), RemoveQuery), responses((status = 204, description = "Service and its machines removed"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
pub(crate) async fn remove(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<RemoveQuery>,
) -> Result<StatusCode> {
    let task = tokio::spawn(async move {
        deploy::remove_service(&state, environment, name, query.purge).await
    });
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
    let backup_schedule = store.backup_schedule(service.id)?;
    let failed_deploy = store
        .list_deployments(service.id)?
        .into_iter()
        .next()
        .filter(|d| d.status == DeploymentStatus::Failed)
        .map(|d| FailedDeploy {
            id: d.id,
            created_at: d.created_at,
        });
    let Some(active) = store.active_deployment(service.id)? else {
        return Ok(ServiceSummary {
            name: service.name,
            state: service.state,
            image: service.image,
            port: service.port,
            replicas: service.replicas,
            memory: service.memory,
            cpus: service.cpus,
            health: service.health,
            health_timeout: service.health_timeout,
            domains,
            volumes,
            backup_schedule,
            deployment: None,
            failed_deploy,
        });
    };
    let machines = store
        .list_machines(active.id)?
        .into_iter()
        .filter(|m| m.state != MachineState::Destroyed)
        .map(|m| MachineSummary {
            id: m.id,
            state: m.state,
            updated_at: m.updated_at,
            stats: None,
        })
        .collect();
    Ok(ServiceSummary {
        name: service.name,
        state: service.state,
        image: active.image,
        port: active.port,
        replicas: service.replicas,
        memory: service.memory,
        cpus: service.cpus,
        health: service.health,
        health_timeout: service.health_timeout,
        domains,
        volumes,
        backup_schedule,
        deployment: Some(DeploymentSummary {
            id: active.id,
            status: active.status,
            created_at: active.created_at,
            machines,
        }),
        failed_deploy,
    })
}
