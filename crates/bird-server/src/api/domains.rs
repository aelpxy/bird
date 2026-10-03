use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::AddDomain;
use bird_core::{Hostname, Name, ServiceId};

use crate::state::AppState;
use crate::{Error, Result, routing};

pub(crate) async fn list(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<Vec<Hostname>>> {
    let service = state.service(&name).await?;
    Ok(Json(hostnames(&state, service.id).await?))
}

pub(crate) async fn add(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(request): Json<AddDomain>,
) -> Result<(StatusCode, Json<Vec<Hostname>>)> {
    let service = state.service(&name).await?;
    let service_id = service.id;
    let hostname = request.hostname;
    let claimed = hostname.clone();
    state
        .db
        .call(move |store| store.add_domain(service_id, &claimed))
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::AlreadyExists(_)) => Error::DomainTaken(hostname),
            other => other,
        })?;
    applied(&state).await?;
    Ok((
        StatusCode::CREATED,
        Json(hostnames(&state, service_id).await?),
    ))
}

pub(crate) async fn remove(
    State(state): State<AppState>,
    Path((name, hostname)): Path<(Name, Hostname)>,
) -> Result<StatusCode> {
    let service = state.service(&name).await?;
    let service_id = service.id;
    let owned = hostnames(&state, service_id).await?.contains(&hostname);
    if !owned {
        return Err(Error::DomainNotFound(hostname));
    }
    state
        .db
        .call(move |store| store.remove_domain(&hostname))
        .await?;
    applied(&state).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn applied(state: &AppState) -> Result<()> {
    routing::refresh(state).await?;
    state.domains_changed.notify_one();
    Ok(())
}

async fn hostnames(state: &AppState, service_id: ServiceId) -> Result<Vec<Hostname>> {
    let domains = state
        .db
        .call(move |store| store.list_domains(service_id))
        .await?;
    Ok(domains.into_iter().map(|d| d.hostname).collect())
}
