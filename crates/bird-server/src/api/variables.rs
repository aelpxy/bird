use axum::Json;
use axum::extract::{Path, State};
use bird_api::{UpdateVariables, VariablesResponse};
use bird_core::{EnvKey, Name, ServiceId};

use crate::state::AppState;
use crate::{Result, deploy};

pub(crate) async fn list(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<Vec<EnvKey>>> {
    let service = state.service(&name).await?;
    Ok(Json(keys(&state, service.id).await?))
}

pub(crate) async fn update(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(update): Json<UpdateVariables>,
) -> Result<Json<VariablesResponse>> {
    // run detached so a disconnecting client cannot abort the redeploy halfway through
    let task = tokio::spawn(async move { apply(&state, &name, update).await });
    match task.await {
        Ok(result) => result.map(Json),
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}

async fn apply(
    state: &AppState,
    name: &Name,
    update: UpdateVariables,
) -> Result<VariablesResponse> {
    let service = state.service(name).await?;
    let service_id = service.id;
    let UpdateVariables { set, unset, deploy } = update;
    state
        .db
        .call(move |store| {
            store.transaction(|store| {
                for (key, value) in &set {
                    store.set_variable(service_id, key, value)?;
                }
                for key in &unset {
                    store.unset_variable(service_id, key)?;
                }
                Ok(())
            })
        })
        .await?;

    let active = state
        .db
        .call(move |store| store.active_deployment(service_id))
        .await?;
    let deployment = match active.filter(|_| deploy) {
        Some(active) => Some(deploy::redeploy(state, &service, active.image, active.port).await?),
        None => None,
    };
    Ok(VariablesResponse {
        keys: keys(state, service_id).await?,
        deployment,
    })
}

async fn keys(state: &AppState, service_id: ServiceId) -> Result<Vec<EnvKey>> {
    let variables = state
        .db
        .call(move |store| store.list_variables(service_id))
        .await?;
    Ok(variables.into_iter().map(|v| v.key).collect())
}
