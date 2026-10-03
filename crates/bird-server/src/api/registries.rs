use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use bird_api::{ErrorBody, RegistryLogin, RegistrySummary};
use bird_core::{Registry, RegistryHost};

use crate::Result;
use crate::state::AppState;

/// List registries bird can pull private images from
#[utoipa::path(get, path = "/v1/registries", tag = "registries", responses((status = 200, description = "Registries with stored credentials; passwords are never returned", body = Vec<RegistrySummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn list(State(state): State<AppState>) -> Result<Json<Vec<RegistrySummary>>> {
    let registries = state.db.call(|store| store.list_registries()).await?;
    Ok(Json(
        registries
            .into_iter()
            .map(|r| RegistrySummary {
                host: r.host,
                username: r.username,
                insecure: r.insecure,
            })
            .collect(),
    ))
}

/// Store credentials for a registry
#[utoipa::path(put, path = "/v1/registries/{host}", tag = "registries", params(("host" = String, Path, description = "Registry host, like ghcr.io")), request_body = RegistryLogin, responses((status = 204, description = "Credentials stored, used for every pull from this host"), (status = 400, description = "Invalid input", body = ErrorBody), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn login(
    State(state): State<AppState>,
    Path(host): Path<RegistryHost>,
    Json(login): Json<RegistryLogin>,
) -> Result<StatusCode> {
    let registry = Registry {
        host: host.clone(),
        username: login.username,
        password: login.password,
        insecure: login.insecure,
    };
    state
        .db
        .call(move |store| store.put_registry(&registry))
        .await?;
    tracing::info!(%host, "registry credentials stored");
    Ok(StatusCode::NO_CONTENT)
}

/// Forget the credentials of a registry
#[utoipa::path(delete, path = "/v1/registries/{host}", tag = "registries", params(("host" = String, Path, description = "Registry host, like ghcr.io")), responses((status = 204, description = "Credentials removed"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "No credentials for this host", body = ErrorBody)))]
pub(crate) async fn logout(
    State(state): State<AppState>,
    Path(host): Path<RegistryHost>,
) -> Result<StatusCode> {
    let removed = host.clone();
    state
        .db
        .call(move |store| store.remove_registry(&removed))
        .await?;
    tracing::info!(%host, "registry credentials removed");
    Ok(StatusCode::NO_CONTENT)
}
