mod guard;
mod machine;
mod remove;

use std::collections::BTreeMap;

use bird_api::{DeployRequest, DeployResponse};
use bird_core::{DeploymentStatus, EnvKey, Hostname, Service};

pub(crate) use guard::DeployGuard;
pub(crate) use machine::set_state;
pub(crate) use remove::remove_service;

use crate::state::AppState;
use crate::{Result, routing};

pub(crate) async fn deploy(state: &AppState, request: DeployRequest) -> Result<DeployResponse> {
    let _ticket = state.deploys.begin(&request.name)?;
    let (service, domains, env) = save_config(state, request).await?;

    let snapshot = service.clone();
    let (deployment, previous) = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let previous = store.active_deployment(snapshot.id)?;
                let deployment = store.create_deployment(&snapshot)?;
                store.set_deployment_status(deployment.id, DeploymentStatus::Deploying)?;
                Ok((deployment, previous))
            })
        })
        .await?;
    tracing::info!(service = %service.name, deployment = %deployment.id, image = %deployment.image, "deploying");

    if let Err(err) = machine::launch(state, &service, &deployment, env).await {
        tracing::warn!(service = %service.name, deployment = %deployment.id, error = %err, "deploy failed");
        let id = deployment.id;
        state
            .db
            .call(move |store| store.set_deployment_status(id, DeploymentStatus::Failed))
            .await?;
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
    tracing::info!(service = %service.name, deployment = %deployment.id, "deployed");

    Ok(DeployResponse {
        service: service.name,
        deployment_id: deployment.id,
        image: deployment.image,
        domains,
    })
}

async fn save_config(
    state: &AppState,
    request: DeployRequest,
) -> Result<(Service, Vec<Hostname>, BTreeMap<EnvKey, String>)> {
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
                let domains = store
                    .list_domains(service.id)?
                    .into_iter()
                    .map(|d| d.hostname)
                    .collect();
                let env = store
                    .list_variables(service.id)?
                    .into_iter()
                    .map(|v| (v.key, v.value))
                    .collect();
                Ok((service, domains, env))
            })
        })
        .await
}
