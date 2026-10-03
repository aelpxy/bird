use std::sync::Arc;

use bird_core::EnvironmentId;
use bird_podman::Podman;
use bird_proxy::Routes;
use tokio::sync::Notify;

use crate::db::Db;
use crate::deploy::DeployGuard;

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Db,
    pub(crate) podman: Podman,
    pub(crate) routes: Routes,
    pub(crate) environment_id: EnvironmentId,
    pub(crate) network: Arc<str>,
    pub(crate) deploys: DeployGuard,
    pub(crate) domains_changed: Arc<Notify>,
}
