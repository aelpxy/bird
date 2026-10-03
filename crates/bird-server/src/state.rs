use std::sync::Arc;

use bird_core::{EnvironmentId, Name, Service};
use bird_podman::Podman;
use bird_proxy::Routes;
use tokio::sync::Notify;

use crate::db::Db;
use crate::deploy::DeployGuard;
use crate::shutdown::Shutdown;
use crate::{Error, Result};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Db,
    pub(crate) podman: Podman,
    pub(crate) routes: Routes,
    pub(crate) environment_id: EnvironmentId,
    pub(crate) network: Arc<str>,
    pub(crate) deploys: DeployGuard,
    pub(crate) domains_changed: Arc<Notify>,
    pub(crate) reconcile_now: Arc<Notify>,
    pub(crate) shutdown: Shutdown,
}

impl AppState {
    pub(crate) async fn service(&self, name: &Name) -> Result<Service> {
        let environment_id = self.environment_id;
        let lookup = name.clone();
        self.db
            .call(move |store| store.service_by_name(environment_id, &lookup))
            .await?
            .ok_or_else(|| Error::ServiceNotFound(name.clone()))
    }
}
