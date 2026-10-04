use std::sync::Arc;

use bird_core::{EnvironmentId, Name, Service};
use bird_podman::Podman;
use bird_proxy::Routes;
use tokio::sync::{Notify, Semaphore};

use crate::backups::BackupStorage;
use crate::cron::CronRuns;
use crate::db::Db;
use crate::deploy::DeployGuard;
use crate::shutdown::Shutdown;
use crate::throttle::LoginThrottle;
use crate::{Error, Result};

#[derive(Clone)]
pub(crate) struct AppState {
    pub(crate) db: Db,
    pub(crate) podman: Podman,
    pub(crate) routes: Routes,
    // the network of environments created before each had its own
    pub(crate) default_network: Arc<str>,
    pub(crate) deploys: DeployGuard,
    pub(crate) domains_changed: Arc<Notify>,
    pub(crate) reconcile_now: Arc<Notify>,
    pub(crate) shutdown: Shutdown,
    pub(crate) builds: Arc<Semaphore>,
    pub(crate) backups: Arc<BackupStorage>,
    // one permit per open terminal, held until its process is hung up
    pub(crate) terminals: Arc<Semaphore>,
    // failed sign-ins per user and address, kept in memory only
    pub(crate) logins: Arc<LoginThrottle>,
    pub(crate) crons: Arc<CronRuns>,
}

impl AppState {
    pub(crate) async fn service(&self, environment: EnvironmentId, name: &Name) -> Result<Service> {
        let lookup = name.clone();
        self.db
            .call(move |store| store.service_by_name(environment, &lookup))
            .await?
            .ok_or_else(|| Error::ServiceNotFound(name.clone()))
    }

    // the podman network an environment's machines share, so their names stay private to it
    pub(crate) async fn network(&self, environment: EnvironmentId) -> Result<String> {
        let found = self
            .db
            .call(move |store| store.environment(environment))
            .await?
            .ok_or(Error::Store(bird_store::Error::NotFound("environment")))?;
        Ok(found
            .network
            .unwrap_or_else(|| self.default_network.to_string()))
    }
}
