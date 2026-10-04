mod check;
mod cleanup;
mod scale;
mod verdict;

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::time::{Duration, Instant};

use bird_core::{Deployment, DeploymentId, EnvKey, MachineId, MachineState, Service, ServiceState};
use tokio::time::{MissedTickBehavior, interval_at};

use crate::backoff::Backoff;
use crate::state::AppState;
use crate::{Result, deploy, routing};

const INTERVAL: Duration = Duration::from_secs(10);

pub(crate) async fn recover_interrupted(state: &AppState) -> Result<()> {
    let interrupted = state.db.call(bird_store::Store::fail_interrupted).await?;
    if interrupted.machines > 0 || interrupted.deployments > 0 {
        tracing::warn!(
            machines = interrupted.machines,
            deployments = interrupted.deployments,
            "marked work interrupted by the last shutdown as failed"
        );
    }
    crate::commands::remove_leftover_runs(state).await;
    Ok(())
}

pub(crate) struct Supervisor {
    state: AppState,
    strikes: HashMap<MachineId, u32>,
    backoff: HashMap<DeploymentId, Backoff>,
}

impl Supervisor {
    pub(crate) fn new(state: AppState) -> Self {
        Self {
            state,
            strikes: HashMap::new(),
            backoff: HashMap::new(),
        }
    }

    pub(crate) async fn run(mut self, shutdown: impl Future<Output = ()>) {
        let mut ticker = interval_at(tokio::time::Instant::now() + INTERVAL, INTERVAL);
        ticker.set_missed_tick_behavior(MissedTickBehavior::Delay);
        let reconcile_now = std::sync::Arc::clone(&self.state.reconcile_now);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                _ = ticker.tick() => {}
                () = reconcile_now.notified() => {}
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = self.sweep() => {}
            }
        }
        tracing::info!("supervisor stopped");
    }

    pub(crate) async fn sweep(&mut self) {
        if let Err(err) = self.supervise_deployments().await {
            tracing::warn!(error = %err, "supervising deployments failed");
        }
        cleanup::retire_machines(&self.state).await;
        cleanup::remove_orphans(&self.state).await;
        if let Err(err) = routing::refresh(&self.state).await {
            tracing::warn!(error = %err, "could not refresh routes");
        }
    }

    async fn supervise_deployments(&mut self) -> Result<()> {
        let deployments = self
            .state
            .db
            .call(|store| store.list_active_deployments())
            .await?;
        let mut seen = HashSet::new();
        for deployment in &deployments {
            let service_id = deployment.service_id;
            let Some(service) = self
                .state
                .db
                .call(move |store| store.service(service_id))
                .await?
            else {
                continue;
            };
            if service.state == ServiceState::Stopped {
                continue;
            }
            let Ok(_ticket) = self.state.deploys.begin(&service.name) else {
                tracing::debug!(service = %service.name, "operation in progress, skipping");
                continue;
            };
            self.supervise(&service, deployment, &mut seen).await?;
        }
        self.strikes.retain(|id, _| seen.contains(id));
        let active: HashSet<DeploymentId> = deployments.iter().map(|d| d.id).collect();
        self.backoff.retain(|id, _| active.contains(id));
        Ok(())
    }

    async fn supervise(
        &mut self,
        service: &Service,
        deployment: &Deployment,
        seen: &mut HashSet<MachineId>,
    ) -> Result<()> {
        let deployment_id = deployment.id;
        let machines = self
            .state
            .db
            .call(move |store| store.list_machines(deployment_id))
            .await?;
        let mut available = Vec::new();
        for machine in machines.iter().filter(|m| m.state == MachineState::Running) {
            seen.insert(machine.id);
            let verdict = check::check_machine(
                &self.state,
                service,
                machine,
                deployment.port,
                &mut self.strikes,
            )
            .await?;
            if verdict.counts_toward_desired() {
                available.push(machine);
            }
        }
        let desired = usize::from(service.replicas.get());
        if let Some(missing) = desired.checked_sub(available.len()).filter(|n| *n > 0) {
            if self.uses_volumes(service).await {
                // a failed machine may still hold the volume, so it has to go before its replacement
                cleanup::retire_machines(&self.state).await;
            }
            self.replace(service, deployment, missing).await;
        } else if let Some(extra) = available.get(desired..) {
            scale::retire_extra(&self.state, service, extra).await;
        }
        Ok(())
    }

    async fn uses_volumes(&self, service: &Service) -> bool {
        let service_id = service.id;
        self.state
            .db
            .call(move |store| store.list_volumes(service_id))
            .await
            .is_ok_and(|volumes| !volumes.is_empty())
    }

    async fn replace(&mut self, service: &Service, deployment: &Deployment, missing: usize) {
        let backoff = self.backoff.entry(deployment.id).or_default();
        if !backoff.ready(Instant::now()) {
            return;
        }
        // replacements run what the deployment was deployed with, not unapplied variable edits
        let deployment_id = deployment.id;
        let env: BTreeMap<EnvKey, String> = match self
            .state
            .db
            .call(move |store| store.deployment_variables(deployment_id))
            .await
        {
            Ok(env) => env,
            Err(err) => {
                tracing::warn!(service = %service.name, error = %err, "could not load variables for replacement");
                return;
            }
        };

        for _ in 0..missing {
            tracing::info!(service = %service.name, deployment = %deployment.id, "launching machine");
            match deploy::launch(&self.state, service, deployment, env.clone()).await {
                Ok(()) => {
                    backoff.reset();
                    tracing::info!(service = %service.name, "machine launched");
                }
                Err(err) => {
                    let wait = backoff.fail(Instant::now());
                    tracing::warn!(service = %service.name, error = %err, retry_in_secs = wait.as_secs(), "machine launch failed");
                    return;
                }
            }
        }
    }
}
