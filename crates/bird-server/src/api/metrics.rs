use axum::Json;
use axum::extract::{Query, State};
use bird_api::{ErrorBody, ServiceMetrics};
use bird_core::{MachineId, MetricsRange};
use serde::Deserialize;

use super::scope::ServiceScope;
use crate::Result;
use crate::login::unix_now;
use crate::state::AppState;

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct MetricsQuery {
    /// How far back: 1h, 6h, 7d or 30d, default 1h
    #[serde(default)]
    #[param(value_type = Option<String>)]
    range: MetricsRange,
    /// Only this machine instead of the whole service
    #[param(value_type = Option<String>)]
    machine: Option<MachineId>,
}

/// Show a service's cpu, memory and traffic over time
#[utoipa::path(get, operation_id = "get_service_metrics", path = "/v1/projects/{project}/environments/{environment}/services/{name}/metrics", tag = "services", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), MetricsQuery), responses((status = 200, description = "Usage in even steps, oldest first", body = ServiceMetrics), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn get(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<MetricsQuery>,
) -> Result<Json<ServiceMetrics>> {
    let service = state.service(environment, &name).await?;
    let range = query.range;
    let step = range.step_secs();
    let since = unix_now() - range.seconds();
    let points = state
        .db
        .call(move |store| store.service_metrics(service.id, since, step, query.machine))
        .await?;
    Ok(Json(ServiceMetrics {
        range,
        step_secs: step,
        points,
    }))
}
