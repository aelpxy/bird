mod guard;
mod machine;
mod recreate;
mod remove;
mod volumes;

use std::collections::BTreeMap;

use bird_api::{DeployRequest, DeployResponse, VolumeSpec};
use bird_core::{Deployment, DeploymentStatus, EnvKey, Hostname, ImageRef, Port, Service, Volume};
use tokio::task::JoinSet;

pub(crate) use guard::DeployGuard;
pub(crate) use machine::{destroy_container, launch, set_state};
pub(crate) use remove::remove_service;

use crate::state::AppState;

// longer than a supervisor replacement takes, so user requests rarely see a busy error
pub(crate) const OPERATION_PATIENCE: std::time::Duration = std::time::Duration::from_mins(2);
use crate::{Result, routing};

pub(crate) async fn redeploy(
    state: &AppState,
    service: &Service,
    image: ImageRef,
    port: Port,
) -> Result<DeployResponse> {
    let request = DeployRequest {
        name: service.name.clone(),
        image,
        port,
        domain: None,
        env: BTreeMap::new(),
        health: None,
        volumes: Vec::new(),
        allow_image_change: false,
    };
    deploy(state, request).await
}

pub(crate) async fn deploy(state: &AppState, request: DeployRequest) -> Result<DeployResponse> {
    let _ticket = state
        .deploys
        .wait_for(&request.name, OPERATION_PATIENCE)
        .await?;
    let new_volumes = volumes::preflight(state, &request).await?;
    let (service, domains, attached) = save_config(state, request, new_volumes).await?;
    state.domains_changed.notify_one();

    let snapshot = service.clone();
    let (deployment, previous, env) = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let previous = store.active_deployment(snapshot.id)?;
                let deployment = store.create_deployment(&snapshot)?;
                store.set_deployment_status(deployment.id, DeploymentStatus::Deploying)?;
                let env = store.deployment_variables(deployment.id)?;
                Ok((deployment, previous, env))
            })
        })
        .await?;
    tracing::info!(service = %service.name, deployment = %deployment.id, image = %deployment.image, "deploying");

    let stopped = if attached.is_empty() {
        Vec::new()
    } else {
        recreate::stop_previous(state, previous.as_ref()).await
    };
    if let Err(err) = launch_all(state, &service, &deployment, &env).await {
        recreate::restore(state, &stopped).await;
        tracing::warn!(service = %service.name, deployment = %deployment.id, error = %err, "deploy failed");
        let id = deployment.id;
        state
            .db
            .call(move |store| store.set_deployment_status(id, DeploymentStatus::Failed))
            .await?;
        // machines that did start belong to a failed deployment now, the supervisor retires them
        state.reconcile_now.notify_one();
        return Err(err);
    }

    let id = deployment.id;
    state
        .db
        .call(move |store| store.activate_deployment(id))
        .await?;
    routing::refresh(state).await?;
    if let Some(previous) = previous {
        machine::retire(state, previous.id).await;
    }
    volumes::record_lineage(state, &attached, &deployment.image).await;
    tracing::info!(service = %service.name, deployment = %deployment.id, "deployed");

    Ok(DeployResponse {
        service: service.name,
        deployment_id: deployment.id,
        image: deployment.image,
        domains,
    })
}

async fn launch_all(
    state: &AppState,
    service: &Service,
    deployment: &Deployment,
    env: &BTreeMap<EnvKey, String>,
) -> Result<()> {
    let mut launches = JoinSet::new();
    for _ in 0..service.replicas.get() {
        let (state, service, deployment, env) = (
            state.clone(),
            service.clone(),
            deployment.clone(),
            env.clone(),
        );
        launches.spawn(async move { machine::launch(&state, &service, &deployment, env).await });
    }
    let mut first_error = None;
    while let Some(joined) = launches.join_next().await {
        let result = joined.unwrap_or_else(|err| Err(std::io::Error::other(err).into()));
        if let Err(err) = result {
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

async fn save_config(
    state: &AppState,
    request: DeployRequest,
    new_volumes: Vec<VolumeSpec>,
) -> Result<(Service, Vec<Hostname>, Vec<Volume>)> {
    let environment_id = state.environment_id;
    state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let service = match store.service_by_name(environment_id, &request.name)? {
                    Some(existing) => {
                        store.update_service(existing.id, &request.image, request.port)?;
                        Service {
                            image: request.image.clone(),
                            port: request.port,
                            ..existing
                        }
                    }
                    None => store.create_service(
                        environment_id,
                        &request.name,
                        &request.image,
                        request.port,
                    )?,
                };
                let service = match request.health {
                    Some(health) => {
                        store.set_health(service.id, health)?;
                        Service { health, ..service }
                    }
                    None => service,
                };
                if let Some(domain) = &request.domain {
                    let owned = store
                        .list_domains(service.id)?
                        .iter()
                        .any(|d| &d.hostname == domain);
                    if !owned {
                        store.add_domain(service.id, domain)?;
                    }
                }
                for (key, value) in &request.env {
                    store.set_variable(service.id, key, value)?;
                }
                for volume in &new_volumes {
                    store.create_volume(service.id, &volume.name, &volume.path)?;
                }
                let domains = store
                    .list_domains(service.id)?
                    .into_iter()
                    .map(|d| d.hostname)
                    .collect();
                let attached = store.list_volumes(service.id)?;
                Ok((service, domains, attached))
            })
        })
        .await
}
