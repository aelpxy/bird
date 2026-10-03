mod check;
mod cleanup;
mod verdict;

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::time::{Duration, Instant};

use bird_core::{Deployment, DeploymentId, MachineId, MachineState, Service};
use tokio::time::{MissedTickBehavior, interval_at};

use crate::backoff::Backoff;
use crate::state::AppState;
use crate::{Result, deploy, routing};

const INTERVAL: Duration = Duration::from_secs(10);
const DESIRED_MACHINES: usize = 1;

pub(crate) async fn recover_interrupted(state: &AppState) -> Result<()> {
    let interrupted = state.db.call(bird_store::Store::fail_interrupted).await?;
    if interrupted.machines > 0 || interrupted.deployments > 0 {
        tracing::warn!(
            machines = interrupted.machines,
            deployments = interrupted.deployments,
            "marked work interrupted by the last shutdown as failed"
        );
    }
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
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                _ = ticker.tick() => {}
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
        let mut available = 0;
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
                available += 1;
            }
        }
        if available < DESIRED_MACHINES {
            self.replace(service, deployment).await;
        }
        Ok(())
    }

    async fn replace(&mut self, service: &Service, deployment: &Deployment) {
        let backoff = self.backoff.entry(deployment.id).or_default();
        if !backoff.ready(Instant::now()) {
            return;
        }
        let service_id = service.id;
        let env = match self
            .state
            .db
            .call(move |store| store.list_variables(service_id))
            .await
        {
            Ok(variables) => variables.into_iter().map(|v| (v.key, v.value)).collect(),
            Err(err) => {
                tracing::warn!(service = %service.name, error = %err, "could not load variables for replacement");
                return;
            }
        };

        tracing::info!(service = %service.name, deployment = %deployment.id, "launching replacement machine");
        match deploy::launch(&self.state, service, deployment, env).await {
            Ok(()) => {
                backoff.reset();
                tracing::info!(service = %service.name, "replacement machine running");
            }
            Err(err) => {
                let wait = backoff.fail(Instant::now());
                tracing::warn!(service = %service.name, error = %err, retry_in_secs = wait.as_secs(), "replacement failed");
            }
        }
    }
}
