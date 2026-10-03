use std::collections::BTreeMap;

use bird_core::reference::{ServiceVariables, referenced_services, resolve};
use bird_core::{EnvKey, Name};

use crate::Result;
use crate::state::AppState;

pub(crate) async fn resolve_for(
    state: &AppState,
    service: &Name,
) -> Result<BTreeMap<EnvKey, String>> {
    let variables = load(state).await?;
    Ok(resolve(service, &variables)?)
}

// resolves as if the change were saved, so a broken reference is refused before anything is stored
pub(crate) async fn check_change(
    state: &AppState,
    service: &Name,
    set: &BTreeMap<EnvKey, String>,
    unset: &[EnvKey],
) -> Result<()> {
    let mut variables = load(state).await?;
    let own = variables.entry(service.clone()).or_default();
    own.extend(set.iter().map(|(k, v)| (k.clone(), v.clone())));
    for key in unset {
        own.remove(key);
    }
    resolve(service, &variables)?;
    let dependents = variables.iter().filter(|(name, vars)| {
        *name != service
            && vars
                .values()
                .any(|value| referenced_services(value).contains(service))
    });
    for (dependent, _) in dependents {
        resolve(dependent, &variables)?;
    }
    Ok(())
}

// services whose variables read from this one, which would break if it disappeared
pub(crate) async fn dependents(state: &AppState, service: &Name) -> Result<Vec<Name>> {
    let variables = load(state).await?;
    Ok(variables
        .into_iter()
        .filter(|(name, vars)| {
            name != service
                && vars
                    .values()
                    .any(|value| referenced_services(value).contains(service))
        })
        .map(|(name, _)| name)
        .collect())
}

async fn load(state: &AppState) -> Result<ServiceVariables> {
    let environment_id = state.environment_id;
    state
        .db
        .call(move |store| {
            store
                .list_services(environment_id)?
                .into_iter()
                .map(|s| {
                    let vars = store
                        .list_variables(s.id)?
                        .into_iter()
                        .map(|v| (v.key, v.value))
                        .collect();
                    Ok((s.name, vars))
                })
                .collect::<bird_store::Result<_>>()
        })
        .await
}
