mod guard;
mod log;
mod machine;
mod output;
mod power;
mod recreate;
pub(crate) mod references;
mod release;
mod remove;
mod settings;
mod volumes;

use std::collections::BTreeMap;

use bird_api::{DeployLogKind, DeployRequest, DeployResponse};
use bird_core::{
    CronJobSpec, Deployment, DeploymentStatus, EnvKey, EnvironmentId, Hostname, ImageRef, Name,
    Port, Service, Volume,
};
use tokio::task::JoinSet;

pub(crate) use guard::DeployGuard;
pub(crate) use log::{DeployLog, RunningLogs, running};
pub(crate) use machine::{MAX_PROCESSES, Pull, destroy_container, ensure_image, launch, set_state};
pub(crate) use power::{restart, start, stop};
pub(crate) use remove::remove_service;

use crate::state::AppState;

// longer than a supervisor replacement takes, so user requests rarely see a busy error
pub(crate) const OPERATION_PATIENCE: std::time::Duration = std::time::Duration::from_mins(2);
use crate::{Result, cron, images, routing, secrets};

pub(crate) async fn redeploy(
    state: &AppState,
    service: &Service,
    image: ImageRef,
    port: Port,
    log: &DeployLog,
) -> Result<DeployResponse> {
    let request = DeployRequest {
        name: service.name.clone(),
        image,
        port,
        domains: Vec::new(),
        env: BTreeMap::new(),
        health: None,
        health_timeout: None,
        command: None,
        release: None,
        memory: None,
        cpus: None,
        volumes: Vec::new(),
        replicas: None,
        backup: None,
        cron: None,
        default_command: false,
        no_release: false,
        allow_image_change: false,
    };
    deploy(state, service.environment_id, request, log).await
}

pub(crate) async fn deploy(
    state: &AppState,
    environment: EnvironmentId,
    mut request: DeployRequest,
    log: &DeployLog,
) -> Result<DeployResponse> {
    if !request.image.has_lowercase_repository() {
        return Err(bird_core::ValidationError::Image(request.image.to_string()).into());
    }
    if request.default_command && request.command.is_some() {
        return Err(crate::Error::CommandConflict);
    }
    if request.no_release && request.release.is_some() {
        return Err(crate::Error::ReleaseConflict);
    }
    if let Some(jobs) = &request.cron {
        cron::check_specs(jobs)?;
    }
    request.env = secrets::expand_all(request.env)?;
    references::check_change(state, environment, &request.name, &request.env, &[]).await?;
    if request.backup.is_some()
        && request.volumes.is_empty()
        && !has_volumes(state, environment, &request.name).await?
    {
        return Err(crate::Error::NothingToBackUp(request.name.clone()));
    }
    let _ticket = state
        .deploys
        .wait_for(environment, &request.name, OPERATION_PATIENCE)
        .await?;
    let new_volumes = volumes::preflight(state, environment, &request).await?;
    let jobs = request.cron.take();
    let (service, previous_settings, domains, attached) =
        settings::save(state, environment, request, new_volumes).await?;
    state.domains_changed.notify_one();

    let resolved = references::resolve_for(state, environment, &service.name).await?;
    let snapshot = service.clone();
    let (deployment, previous, env) = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let previous = store.active_deployment(snapshot.id)?;
                let deployment = store.create_deployment(&snapshot)?;
                store.set_deployment_status(deployment.id, DeploymentStatus::Deploying)?;
                store.set_resolved_variables(deployment.id, &resolved)?;
                let env = store.deployment_variables(deployment.id)?;
                Ok((deployment, previous, env))
            })
        })
        .await?;
    tracing::info!(service = %service.name, deployment = %deployment.id, image = %deployment.image, "deploying");
    log.register(state, deployment.id);
    let (service_id, deployment_id) = (service.id, deployment.id);
    let rollout = Rollout {
        service,
        previous_settings,
        domains,
        attached,
        deployment,
        previous,
        env,
        jobs,
    };
    let result = roll_out(state, rollout, log).await;
    if let Err(err) = &result {
        log.bird(format!(
            "The deploy failed: {err}. The previous deployment keeps serving."
        ));
    }
    log.finish(state, service_id, deployment_id).await;
    result
}

// a saved deployment on its way to serving traffic
struct Rollout {
    service: Service,
    previous_settings: Option<Service>,
    domains: Vec<Hostname>,
    attached: Vec<Volume>,
    deployment: Deployment,
    previous: Option<Deployment>,
    env: BTreeMap<EnvKey, String>,
    jobs: Option<Vec<CronJobSpec>>,
}

async fn roll_out(state: &AppState, rollout: Rollout, log: &DeployLog) -> Result<DeployResponse> {
    let Rollout {
        service,
        previous_settings,
        domains,
        attached,
        deployment,
        previous,
        env,
        jobs,
    } = rollout;
    let started = std::time::Instant::now();
    for line in state.built.take(&deployment.image) {
        log.push(DeployLogKind::Build, None, line);
    }
    log.bird(format!(
        "Deploying {} as deployment {}",
        deployment.image, deployment.id
    ));

    let released = match pull(state, &deployment.image, log).await {
        Ok(()) => match &service.release {
            Some(release) => release::run(state, &service, &deployment, &env, release, log).await,
            None => Ok(()),
        },
        Err(err) => Err(err),
    };
    let mut stopped = Vec::new();
    let launched = match released {
        Ok(()) => {
            if !attached.is_empty() {
                log.bird("Stopping the previous machine first, since only one may use the volumes");
                stopped = recreate::stop_previous(state, previous.as_ref()).await;
            }
            launch_all(state, &service, &deployment, &env, log).await
        }
        Err(err) => Err(err),
    };
    if let Err(err) = launched {
        recreate::restore(state, &stopped).await;
        settings::restore(state, previous_settings).await;
        tracing::warn!(service = %service.name, deployment = %deployment.id, error = %err, "deploy failed");
        let id = deployment.id;
        state
            .db
            .call(move |store| store.set_deployment_status(id, DeploymentStatus::Failed))
            .await?;
        // machines that did start belong to a failed deployment now, the supervisor retires them
        state.reconcile_now.notify_one();
        return Err(err);
    }

    let id = deployment.id;
    log.bird("Moving traffic to the new machines");
    state
        .db
        .call(move |store| store.activate_deployment(id))
        .await?;
    routing::refresh(state).await?;
    if let Some(previous) = previous {
        log.bird("Stopping the previous machines");
        machine::retire(state, previous.id).await;
    }
    // jobs change with a deploy that worked, so a failed one keeps running the old ones
    if let Some(jobs) = jobs {
        let count = cron::set_jobs(state, service.id, jobs).await?.len();
        tracing::info!(service = %service.name, jobs = count, "cron jobs set");
    }
    volumes::record_lineage(state, &attached, &deployment.image).await;
    let cleanup = state.clone();
    tokio::spawn(async move { images::remove_unused(&cleanup).await });
    tracing::info!(service = %service.name, deployment = %deployment.id, "deployed");
    log.bird(format!(
        "Deployed in {:.1}s",
        started.elapsed().as_secs_f32()
    ));

    Ok(DeployResponse {
        service: service.name,
        deployment_id: deployment.id,
        image: deployment.image,
        domains,
    })
}

// a pulled image is reported once, before machines or the release command need it
async fn pull(state: &AppState, image: &ImageRef, log: &DeployLog) -> Result<()> {
    if image.is_local() {
        log.bird(format!("Using the image built on this server, {image}"));
        return ensure_image(state, image).await;
    }
    log.bird(format!("Pulling {image}"));
    let started = std::time::Instant::now();
    ensure_image(state, image).await?;
    log.bird(format!("Pulled in {:.1}s", started.elapsed().as_secs_f32()));
    Ok(())
}

async fn launch_all(
    state: &AppState,
    service: &Service,
    deployment: &Deployment,
    env: &BTreeMap<EnvKey, String>,
    log: &DeployLog,
) -> Result<()> {
    let replicas = service.replicas.get();
    log.bird(if replicas == 1 {
        "Starting 1 machine".to_owned()
    } else {
        format!("Starting {replicas} machines")
    });
    let mut launches = JoinSet::new();
    for _ in 0..replicas {
        let (state, service, deployment, env, log) = (
            state.clone(),
            service.clone(),
            deployment.clone(),
            env.clone(),
            log.clone(),
        );
        launches.spawn(async move {
            machine::launch(&state, &service, &deployment, env, Pull::Done, &log).await
        });
    }
    let mut first_error = None;
    while let Some(joined) = launches.join_next().await {
        let result = joined.unwrap_or_else(|err| Err(std::io::Error::other(err).into()));
        if let Err(err) = result {
            first_error.get_or_insert(err);
        }
    }
    first_error.map_or(Ok(()), Err)
}

async fn has_volumes(state: &AppState, environment_id: EnvironmentId, name: &Name) -> Result<bool> {
    let name = name.clone();
    state
        .db
        .call(move |store| {
            let Some(service) = store.service_by_name(environment_id, &name)? else {
                return Ok(false);
            };
            Ok(!store.list_volumes(service.id)?.is_empty())
        })
        .await
}
