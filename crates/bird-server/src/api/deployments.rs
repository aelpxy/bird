use axum::Json;
use axum::extract::{Path, State};
use bird_api::ErrorBody;
use bird_api::{DeployResponse, DeploymentInfo, RollbackRequest};
use bird_core::{Deployment, DeploymentStatus, Name, ServiceId};

use crate::state::AppState;
use crate::{Error, Result, deploy};

/// List deployments
#[utoipa::path(get, path = "/v1/services/{name}/deployments", tag = "deployments", params(("name" = String, Path, description = "Service name")), responses((status = 200, description = "Deployments, newest first", body = Vec<DeploymentInfo>), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody)))]
pub(crate) async fn list(
    State(state): State<AppState>,
    Path(name): Path<Name>,
) -> Result<Json<Vec<DeploymentInfo>>> {
    let service = state.service(&name).await?;
    let deployments = history(&state, service.id).await?;
    let ids: Vec<_> = deployments.iter().map(|d| d.id).collect();
    let counts = state
        .db
        .call(move |store| {
            ids.iter()
                .map(|id| Ok(store.deployment_variables(*id)?.len()))
                .collect::<bird_store::Result<Vec<_>>>()
        })
        .await?;
    Ok(Json(
        deployments.into_iter().zip(counts).map(info).collect(),
    ))
}

/// Roll back to an earlier deployment
#[utoipa::path(post, path = "/v1/services/{name}/rollback", tag = "deployments", params(("name" = String, Path, description = "Service name")), request_body = RollbackRequest, responses((status = 200, description = "Earlier image, port and variables deployed again", body = DeployResponse), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or resource not found", body = ErrorBody), (status = 409, description = "Conflicting state or operation in progress", body = ErrorBody), (status = 502, description = "The app did not become healthy; includes its last log lines", body = ErrorBody)))]
pub(crate) async fn rollback(
    State(state): State<AppState>,
    Path(name): Path<Name>,
    Json(request): Json<RollbackRequest>,
) -> Result<Json<DeployResponse>> {
    // run detached so a disconnecting client cannot abort the redeploy halfway through
    let task = tokio::spawn(async move {
        let service = state.service(&name).await?;
        let deployments = history(&state, service.id).await?;
        let target = pick_target(&deployments, request.deployment_id, &name)?;
        tracing::info!(service = %name, target = %target.id, image = %target.image, "rolling back");
        let (service_id, target_id) = (service.id, target.id);
        state
            .db
            .call(move |store| store.restore_variables(service_id, target_id))
            .await?;
        deploy::redeploy(&state, &service, target.image.clone(), target.port).await
    });
    match task.await {
        Ok(result) => result.map(Json),
        Err(err) => Err(std::io::Error::other(err).into()),
    }
}

async fn history(state: &AppState, service_id: ServiceId) -> Result<Vec<Deployment>> {
    state
        .db
        .call(move |store| store.list_deployments(service_id))
        .await
}

fn pick_target<'a>(
    deployments: &'a [Deployment],
    requested: Option<bird_core::DeploymentId>,
    name: &Name,
) -> Result<&'a Deployment> {
    match requested {
        Some(id) => {
            let target = deployments
                .iter()
                .find(|d| d.id == id)
                .ok_or(Error::DeploymentNotFound(id))?;
            match target.status {
                DeploymentStatus::Active | DeploymentStatus::Superseded => Ok(target),
                status => Err(Error::NotRollbackable { id, status }),
            }
        }
        None => deployments
            .iter()
            .find(|d| d.status == DeploymentStatus::Superseded)
            .ok_or_else(|| Error::NoRollbackTarget(name.clone())),
    }
}

fn info((deployment, variables): (Deployment, usize)) -> DeploymentInfo {
    DeploymentInfo {
        id: deployment.id,
        image: deployment.image,
        port: deployment.port,
        status: deployment.status,
        variables,
        created_at: deployment.created_at,
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{DeploymentId, ServiceId};

    use super::*;

    fn deployment(status: DeploymentStatus, image: &str) -> Deployment {
        Deployment {
            id: DeploymentId::generate(),
            service_id: ServiceId::generate(),
            image: image.parse().unwrap(),
            port: bird_core::Port::try_from(80).unwrap(),
            status,
            created_at: 0,
        }
    }

    #[test]
    fn defaults_to_newest_superseded_deployment() {
        let newest_first = [
            deployment(DeploymentStatus::Failed, "app:4"),
            deployment(DeploymentStatus::Active, "app:3"),
            deployment(DeploymentStatus::Superseded, "app:2"),
            deployment(DeploymentStatus::Superseded, "app:1"),
        ];
        let name = "web".parse().unwrap();
        let target = pick_target(&newest_first, None, &name).unwrap();
        assert_eq!(target.image.as_str(), "app:2");
    }

    #[test]
    fn rejects_failed_unknown_or_missing_targets() {
        let failed = deployment(DeploymentStatus::Failed, "app:2");
        let active = deployment(DeploymentStatus::Active, "app:1");
        let name = "web".parse().unwrap();
        let only = [failed.clone(), active.clone()];
        assert!(matches!(
            pick_target(&only, Some(failed.id), &name),
            Err(Error::NotRollbackable { .. })
        ));
        assert!(matches!(
            pick_target(&only, Some(DeploymentId::generate()), &name),
            Err(Error::DeploymentNotFound(_))
        ));
        assert!(matches!(
            pick_target(&only, None, &name),
            Err(Error::NoRollbackTarget(_))
        ));
        assert_eq!(
            pick_target(&only, Some(active.id), &name).unwrap().id,
            active.id
        );
    }
}
