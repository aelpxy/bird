use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::AddDomain;
use bird_api::ErrorBody;
use bird_core::{Hostname, Name, ServiceId};

use crate::state::AppState;
use crate::{Error, Result, routing};

/// List domains of a service
#[utoipa::path(get, path = "/v1/services/{name}/domains", tag = "domains", params(("name" = String, Path, description = "Service name")), responses((status = 200, description = "Domains routed to the service", body = Vec<String>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<Vec<Hostname>>> {
    let service = state.service(&name).await?;
    Ok(Json(hostnames(&state, service.id).await?))
}

/// Route a domain to a service
#[utoipa::path(post, path = "/v1/services/{name}/domains", tag = "domains", params(("name" = String, Path, description = "Service name")), request_body = AddDomain, responses((status = 201, description = "Domain added, returns all domains of the service", body = Vec<String>), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody)))]
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

/// Stop routing a domain to a service
#[utoipa::path(delete, path = "/v1/services/{name}/domains/{hostname}", tag = "domains", params(("name" = String, Path, description = "Service name"), ("hostname" = String, Path, description = "Domain to remove")), responses((status = 204, description = "Domain removed"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
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
