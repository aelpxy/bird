use axum::Json;
use axum::extract::{Path, State};
use bird_api::{CreateFromTemplate, ErrorBody, TemplateDeployResponse, TemplateSummary};
use bird_core::Name;

use crate::state::AppState;
use crate::{Error, Result, deploy, templates};

/// List built-in templates
#[utoipa::path(get, path = "/v1/templates", tag = "templates", responses((status = 200, description = "Templates that can create a service", body = Vec<TemplateSummary>), (status = 401, description = "Missing or invalid API token", body = ErrorBody)))]
pub(crate) async fn list() -> Result<Json<Vec<TemplateSummary>>> {
    let templates = templates::built_in()?;
    Ok(Json(
        templates.iter().map(templates::Template::summary).collect(),
    ))
}

/// Create a service from a template
#[utoipa::path(post, path = "/v1/templates/{template}/deploy", tag = "templates", params(("template" = String, Path, description = "Template name, like postgres")), request_body = CreateFromTemplate, responses((status = 200, description = "Service created and running", body = TemplateDeployResponse), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Unknown template", body = ErrorBody), (status = 409, description = "A service with that name already exists", body = ErrorBody), (status = 502, description = "The service did not become healthy", body = ErrorBody)))]
pub(crate) async fn deploy(
    State(state): State<AppState>,
    Path(template): Path<Name>,
    Json(request): Json<CreateFromTemplate>,
) -> Result<Json<TemplateDeployResponse>> {
    let template = templates::find(&template)?;
    let name = request.name.unwrap_or_else(|| template.name().clone());
    let environment_id = state.environment_id;
    let lookup = name.clone();
    let taken = state
        .db
        .call(move |store| store.service_by_name(environment_id, &lookup))
        .await?
        .is_some();
    if taken {
        return Err(Error::ServiceExists {
            name,
            template: template.name().clone(),
        });
    }
    // run detached so a disconnecting client cannot abort the deploy halfway through
    let task = tokio::spawn(async move {
        let deployment = deploy::deploy(&state, template.request(&name)).await?;
        Ok(TemplateDeployResponse {
            deployment,
            connection: template.connection().cloned(),
        })
    });
    match task.await {
        Ok(result) => result.map(Json),
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}
