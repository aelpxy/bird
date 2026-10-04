use axum::extract::{FromRequestParts, RawPathParams};
use axum::http::request::Parts;
use bird_core::{EnvironmentId, Name};

use crate::state::AppState;
use crate::{Error, Result, environments};

// the environment a request acts in, from `/v1/projects/{project}/environments/{environment}/..`
pub(crate) struct Scope(pub(crate) EnvironmentId);

// a service in that environment, from `../services/{name}`
pub(crate) struct ServiceScope {
    pub(crate) environment: EnvironmentId,
    pub(crate) name: Name,
}

impl FromRequestParts<AppState> for Scope {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let params = params(parts, state).await?;
        let project = param(&params, "project")?;
        let environment = param(&params, "environment")?;
        Ok(Self(
            environments::find(state, &project, &environment).await?.id,
        ))
    }
}

impl FromRequestParts<AppState> for ServiceScope {
    type Rejection = Error;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self> {
        let name = param(&params(parts, state).await?, "name")?;
        let Scope(environment) = Scope::from_request_parts(parts, state).await?;
        Ok(Self { environment, name })
    }
}

async fn params(parts: &mut Parts, state: &AppState) -> Result<RawPathParams> {
    RawPathParams::from_request_parts(parts, state)
        .await
        .map_err(|err| Error::InvalidPath(err.body_text()))
}

fn param(params: &RawPathParams, key: &str) -> Result<Name> {
    let raw = params
        .iter()
        .find_map(|(name, value)| (name == key).then_some(value))
        .ok_or_else(|| Error::InvalidPath(format!("no {key} in the path")))?;
    Ok(raw.parse()?)
}
