use std::collections::BTreeSet;

use bird_api::{DeployRequest, VolumeSpec};
use bird_core::{EnvironmentId, Hostname, Service, ServiceState, Volume};
use bird_store::Store;

use crate::state::AppState;
use crate::{Error, Result};

// a failed deploy must not leave its image, port, command or limits for the next one to inherit
pub(super) async fn restore(state: &AppState, previous: Option<Service>) {
    let Some(previous) = previous else {
        return;
    };
    let name = previous.name.clone();
    let restored = state
        .db
        .call(move |store| store.restore_settings(&previous))
        .await;
    if let Err(err) = restored {
        tracing::error!(service = %name, error = %err, "could not restore service settings");
    }
}

pub(super) async fn save(
    state: &AppState,
    environment_id: EnvironmentId,
    request: DeployRequest,
    new_volumes: Vec<VolumeSpec>,
) -> Result<(Service, Option<Service>, Vec<Hostname>, Vec<Volume>)> {
    if let Some(dashboard) = state
        .dashboard_domain
        .as_ref()
        .filter(|dashboard| request.domains.contains(dashboard))
    {
        return Err(Error::DashboardDomain(dashboard.clone()));
    }
    state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let previous = store.service_by_name(environment_id, &request.name)?;
                let service = match previous.clone() {
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
                let service = apply_settings(store, service, &request)?;
                let mut owned: BTreeSet<Hostname> = store
                    .list_domains(service.id)?
                    .into_iter()
                    .map(|d| d.hostname)
                    .collect();
                for domain in &request.domains {
                    if owned.insert(domain.clone()) {
                        store.add_domain(service.id, domain)?;
                    }
                }
                for (key, value) in &request.env {
                    store.set_variable(service.id, key, value)?;
                }
                for volume in &new_volumes {
                    store.create_volume(service.id, &volume.name, &volume.path)?;
                }
                if let Some(schedule) = request.backup {
                    store.set_backup_schedule(service.id, schedule)?;
                }
                let domains = store
                    .list_domains(service.id)?
                    .into_iter()
                    .map(|d| d.hostname)
                    .collect();
                let attached = store.list_volumes(service.id)?;
                Ok((service, previous, domains, attached))
            })
        })
        .await
}

// the settings a deploy may change; anything left out of the request keeps its saved value
fn apply_settings(
    store: &Store,
    service: Service,
    request: &DeployRequest,
) -> bird_store::Result<Service> {
    // deploying means running, also for a service that was stopped
    let service = if service.state == ServiceState::Stopped {
        store.set_service_state(service.id, ServiceState::Running)?;
        Service {
            state: ServiceState::Running,
            ..service
        }
    } else {
        service
    };
    let service = match &request.health {
        Some(health) => {
            store.set_health(service.id, health)?;
            Service {
                health: health.clone(),
                ..service
            }
        }
        None => service,
    };
    let service = match request.health_timeout {
        Some(health_timeout) => {
            store.set_health_timeout(service.id, health_timeout)?;
            Service {
                health_timeout,
                ..service
            }
        }
        None => service,
    };
    let service = if request.memory.is_some() || request.cpus.is_some() {
        let memory = request.memory.unwrap_or(service.memory);
        let cpus = request.cpus.unwrap_or(service.cpus);
        store.set_resources(service.id, memory, cpus)?;
        Service {
            memory,
            cpus,
            ..service
        }
    } else {
        service
    };
    let service = match request.replicas {
        Some(replicas) => {
            store.set_replicas(service.id, replicas)?;
            Service {
                replicas,
                ..service
            }
        }
        None => service,
    };
    let service = match &request.command {
        Some(command) => {
            store.set_command(service.id, Some(command))?;
            Service {
                command: Some(command.clone()),
                ..service
            }
        }
        None if request.default_command => {
            store.set_command(service.id, None)?;
            Service {
                command: None,
                ..service
            }
        }
        None => service,
    };
    let service = match &request.release {
        Some(release) => {
            store.set_release(service.id, Some(release))?;
            Service {
                release: Some(release.clone()),
                ..service
            }
        }
        None if request.no_release => {
            store.set_release(service.id, None)?;
            Service {
                release: None,
                ..service
            }
        }
        None => service,
    };
    Ok(service)
}
