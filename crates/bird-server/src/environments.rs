use bird_core::{Environment, Name, Project};

use crate::state::AppState;
use crate::{Error, Result};

// every project starts with one, and commands act in it unless told otherwise
pub(crate) const FIRST_ENVIRONMENT: &str = "production";

// what a scoped request names, as `<project>/<environment>`
pub(crate) async fn find(
    state: &AppState,
    project: &Name,
    environment: &Name,
) -> Result<Environment> {
    let (lookup_project, lookup_environment) = (project.clone(), environment.clone());
    let found = state
        .db
        .call(move |store| {
            let Some(project) = store.project_by_name(&lookup_project)? else {
                return Ok(None);
            };
            Ok(Some(
                store.environment_by_name(project.id, &lookup_environment)?,
            ))
        })
        .await?;
    match found {
        None => Err(Error::ProjectNotFound(project.clone())),
        Some(None) => Err(Error::EnvironmentNotFound {
            project: project.clone(),
            environment: environment.clone(),
        }),
        Some(Some(environment)) => Ok(environment),
    }
}

pub(crate) async fn list(state: &AppState) -> Result<Vec<(Project, Vec<Environment>)>> {
    state
        .db
        .call(|store| {
            store
                .list_projects()?
                .into_iter()
                .map(|project| {
                    let environments = store.list_environments(project.id)?;
                    Ok((project, environments))
                })
                .collect()
        })
        .await
}

pub(crate) async fn create_project(state: &AppState, name: &Name) -> Result<Environment> {
    let first: Name = FIRST_ENVIRONMENT.parse()?;
    let network = network_name(name, &first);
    let (project, owned_network) = (name.clone(), network.clone());
    let environment = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let project = store.create_project(&project)?;
                store.create_environment(project.id, &first, Some(&owned_network))
            })
        })
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::AlreadyExists(_)) => Error::ProjectExists(name.clone()),
            other => other,
        })?;
    if let Err(err) = state.podman.ensure_network(&network).await {
        let project = environment.project_id;
        state
            .db
            .call(move |store| store.delete_project(project))
            .await?;
        return Err(err.into());
    }
    tracing::info!(project = %name, "project created");
    Ok(environment)
}

pub(crate) async fn create_environment(
    state: &AppState,
    project: &Name,
    name: &Name,
) -> Result<Environment> {
    let network = network_name(project, name);
    let (lookup, owned_name, owned_network) = (project.clone(), name.clone(), network.clone());
    let created = state
        .db
        .call(move |store| {
            let Some(project) = store.project_by_name(&lookup)? else {
                return Ok(None);
            };
            store
                .create_environment(project.id, &owned_name, Some(&owned_network))
                .map(Some)
        })
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::AlreadyExists(_)) => Error::EnvironmentExists {
                project: project.clone(),
                environment: name.clone(),
            },
            other => other,
        })?;
    let Some(environment) = created else {
        return Err(Error::ProjectNotFound(project.clone()));
    };
    if let Err(err) = state.podman.ensure_network(&network).await {
        let id = environment.id;
        state
            .db
            .call(move |store| store.delete_environment(id))
            .await?;
        return Err(err.into());
    }
    tracing::info!(project = %project, environment = %name, "environment created");
    Ok(environment)
}

// refused while services or backups are left, checked in the same transaction as the delete so a
// deploy cannot slip in between
pub(crate) async fn remove_environment(
    state: &AppState,
    project: &Name,
    name: &Name,
) -> Result<()> {
    let environment = find(state, project, name).await?;
    let id = environment.id;
    let leftover = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let contents = store.environment_contents(id)?;
                if contents.0.is_empty() && contents.1.is_empty() {
                    store.delete_environment(id)?;
                }
                Ok(contents)
            })
        })
        .await?;
    refuse_leftovers(&format!("{project}/{name}"), leftover)?;
    remove_network(state, environment.network.as_deref()).await;
    tracing::info!(project = %project, environment = %name, "environment removed");
    Ok(())
}

pub(crate) async fn remove_project(state: &AppState, name: &Name) -> Result<()> {
    let lookup = name.clone();
    let outcome = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let Some(project) = store.project_by_name(&lookup)? else {
                    return Ok(None);
                };
                let environments = store.list_environments(project.id)?;
                for environment in &environments {
                    let contents = store.environment_contents(environment.id)?;
                    if !contents.0.is_empty() || !contents.1.is_empty() {
                        return Ok(Some(Err((environment.name.clone(), contents))));
                    }
                }
                store.delete_project(project.id)?;
                Ok(Some(Ok(environments)))
            })
        })
        .await?;
    let Some(outcome) = outcome else {
        return Err(Error::ProjectNotFound(name.clone()));
    };
    let environments = match outcome {
        Ok(environments) => environments,
        Err((environment, leftover)) => {
            return refuse_leftovers(&format!("{name}/{environment}"), leftover);
        }
    };
    for environment in &environments {
        remove_network(state, environment.network.as_deref()).await;
    }
    tracing::info!(project = %name, "project removed");
    Ok(())
}

// networks of environments made before each had its own are left in place at startup too
pub(crate) async fn ensure_networks(state: &AppState) -> Result<()> {
    let environments = state.db.call(|store| store.list_all_environments()).await?;
    for network in environments.iter().filter_map(|env| env.network.as_deref()) {
        state.podman.ensure_network(network).await?;
    }
    Ok(())
}

// names never contain `_`, so no two environments can map to the same network
fn network_name(project: &Name, environment: &Name) -> String {
    format!("bird_{project}_{environment}")
}

fn refuse_leftovers(scope: &str, (services, backups): (Vec<Name>, Vec<Name>)) -> Result<()> {
    if !services.is_empty() {
        return Err(Error::EnvironmentHasServices {
            scope: scope.to_owned(),
            services,
        });
    }
    if !backups.is_empty() {
        return Err(Error::EnvironmentHasBackups {
            scope: scope.to_owned(),
            services: backups,
        });
    }
    Ok(())
}

// the shared network birdd was started with stays; one podman still uses is only logged
async fn remove_network(state: &AppState, network: Option<&str>) {
    let Some(network) = network else {
        return;
    };
    match state.podman.remove_network(network).await {
        Ok(()) | Err(bird_podman::Error::NotFound { .. }) => {}
        Err(err) => tracing::warn!(network, error = %err, "could not remove environment network"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn networks_cannot_collide() {
        let name = |raw: &str| raw.parse::<Name>().unwrap();
        assert_eq!(
            network_name(&name("shop"), &name("staging")),
            "bird_shop_staging"
        );
        assert_ne!(
            network_name(&name("a-b"), &name("c")),
            network_name(&name("a"), &name("b-c"))
        );
    }
}
