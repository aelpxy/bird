use axum::Json;
use axum::extract::State;
use bird_api::ErrorBody;
use bird_core::ProxySettings;

use super::scope::ServiceScope;
use crate::state::AppState;
use crate::{Result, routing};

/// How the proxy treats the service's requests
#[utoipa::path(get, operation_id = "get_proxy_settings", path = "/v1/projects/{project}/environments/{environment}/services/{name}/proxy", tag = "services", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name")), responses((status = 200, description = "The service's proxy settings", body = ProxySettings), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn get(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
) -> Result<Json<ProxySettings>> {
    let service = state.service(environment, &name).await?;
    let settings = state
        .db
        .call(move |store| store.proxy_settings(service.id))
        .await?;
    Ok(Json(settings))
}

/// Replace the service's proxy settings; they apply to the next request
#[utoipa::path(put, operation_id = "set_proxy_settings", path = "/v1/projects/{project}/environments/{environment}/services/{name}/proxy", tag = "services", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name")), request_body = ProxySettings, responses((status = 200, description = "Saved and in effect", body = ProxySettings), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn set(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Json(settings): Json<ProxySettings>,
) -> Result<Json<ProxySettings>> {
    let service = state.service(environment, &name).await?;
    let saved = settings.clone();
    state
        .db
        .call(move |store| store.set_proxy_settings(service.id, &saved))
        .await?;
    routing::refresh(&state).await?;
    tracing::info!(service = %name, maintenance = settings.maintenance, "proxy settings changed");
    Ok(Json(settings))
}
