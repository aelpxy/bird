use std::collections::{BTreeMap, HashSet};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use bird_core::{Command, EnvKey, EnvironmentId, ImageRef, Name, Service};
use bird_podman::{ContainerSpec, Lifecycle, Limits};
use bytes::Bytes;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::sync::mpsc;

use super::forward;
use super::tty::Stdio;
use crate::deploy::{MAX_PROCESSES, ensure_image};
use crate::state::AppState;
use crate::{Error, Result};

const RUN_TIMEOUT: Duration = Duration::from_hours(1);
const RUN_LABEL: &str = "bird.run";
// not the machines' environment label, so the orphan sweep still leaves running commands alone
const RUN_ENVIRONMENT_LABEL: &str = "bird.run-environment";

pub(crate) struct RunTarget {
    service: Service,
    image: ImageRef,
    env: BTreeMap<EnvKey, String>,
    network: String,
    skip_entrypoint: bool,
}

impl RunTarget {
    // by default the command goes to the image's entrypoint as arguments, like `docker run`
    pub(crate) fn skipping_entrypoint(self, skip: bool) -> Self {
        Self {
            skip_entrypoint: skip,
            ..self
        }
    }
}

// the active deployment, or the newest one when none succeeded yet, like a first deploy that needs setup
pub(crate) async fn run_target(
    state: &AppState,
    environment: EnvironmentId,
    name: &Name,
) -> Result<RunTarget> {
    let service = state.service(environment, name).await?;
    let service_id = service.id;
    let found = state
        .db
        .call(move |store| {
            let deployment = match store.active_deployment(service_id)? {
                Some(active) => Some(active),
                None => store.list_deployments(service_id)?.into_iter().next(),
            };
            deployment
                .map(|d| Ok((d.image, store.deployment_variables(d.id)?)))
                .transpose()
        })
        .await?;
    let Some((image, env)) = found else {
        return Err(Error::NeverDeployed(name.clone()));
    };
    Ok(RunTarget {
        network: state.network(environment).await?,
        skip_entrypoint: false,
        service,
        image,
        env,
    })
}

pub(crate) async fn run(
    state: &AppState,
    target: &RunTarget,
    command: &Command,
    events: &mpsc::Sender<Bytes>,
) -> Result<Option<i32>> {
    run_with_limit(state, target, command, events, RUN_TIMEOUT).await
}

// the container is removed however this ends: exit, disconnect, timeout or shutdown
pub(crate) async fn run_with_limit(
    state: &AppState,
    target: &RunTarget,
    command: &Command,
    events: &mpsc::Sender<Bytes>,
    limit: Duration,
) -> Result<Option<i32>> {
    ensure_image(state, &target.image).await?;
    let spec = spec(target, command, Lifecycle::OneOff);
    let id = state.podman.create_container(&spec).await?;
    tracing::info!(service = %target.service.name, container = %spec.name, "running one-off command");
    let outcome = tokio::select! {
        () = state.shutdown.wait() => Err(Error::ShuttingDown),
        () = events.closed() => Ok(None),
        result = tokio::time::timeout(limit, follow(state, &id, limit, events)) => {
            result.unwrap_or_else(|_| Err(Error::RunTimedOut(limit)))
        }
    };
    if let Err(err) = state.podman.remove_container(&id).await {
        tracing::warn!(container = %spec.name, error = %err, "could not remove one-off container");
    }
    outcome
}

async fn follow(
    state: &AppState,
    id: &str,
    limit: Duration,
    events: &mpsc::Sender<Bytes>,
) -> Result<Option<i32>> {
    state.podman.start_container(id).await?;
    let mut output = state.podman.follow_output(id).await?;
    if !forward(&mut output, events).await? {
        return Ok(None);
    }
    Ok(Some(state.podman.wait_container(id, limit).await?))
}

// attached before it starts, so the first output and the first prompt reach the client
pub(crate) async fn start_attached(
    state: &AppState,
    target: &RunTarget,
    command: &Command,
    stdio: Stdio,
) -> Result<(String, TokioIo<Upgraded>)> {
    ensure_image(state, &target.image).await?;
    let lifecycle = match stdio {
        Stdio::Terminal { .. } => Lifecycle::Terminal,
        Stdio::Piped => Lifecycle::Piped,
    };
    let spec = spec(target, command, lifecycle);
    let id = state.podman.create_container(&spec).await?;
    let attached = async {
        let io = state.podman.attach(&id).await?;
        state.podman.start_container(&id).await?;
        if let Stdio::Terminal { cols, rows } = stdio
            && let Err(err) = state.podman.resize_container(&id, cols, rows).await
        {
            tracing::debug!(error = %err, "could not size the new terminal");
        }
        Ok::<_, Error>(io)
    }
    .await;
    match attached {
        Ok(io) => {
            tracing::info!(service = %target.service.name, container = %spec.name, "running attached one-off command");
            Ok((id, io))
        }
        Err(err) => {
            if let Err(cleanup) = state.podman.remove_container(&id).await {
                tracing::warn!(container = %spec.name, error = %cleanup, "could not remove one-off container");
            }
            Err(err)
        }
    }
}

fn spec(target: &RunTarget, command: &Command, lifecycle: Lifecycle) -> ContainerSpec {
    let service = &target.service;
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis());
    let (entrypoint, command) = match command.args().split_first() {
        Some((program, args)) if target.skip_entrypoint => {
            (Some(vec![program.clone()]), Some(args.to_vec()))
        }
        _ => (None, Some(command.args().to_vec())),
    };
    ContainerSpec {
        name: format!("bird-run-{}-{millis}", service.name),
        image: target.image.clone(),
        command,
        entrypoint,
        lifecycle,
        network: target.network.clone(),
        aliases: Vec::new(),
        mounts: Vec::new(),
        limits: Limits {
            memory_bytes: service.memory.bytes(),
            cpu_millicores: service.cpus.millicores(),
            pids: MAX_PROCESSES,
        },
        env: target.env.clone(),
        // no environment label, so the supervisor's orphan sweep leaves running commands alone
        labels: BTreeMap::from([
            ("bird.managed".to_owned(), "true".to_owned()),
            (RUN_LABEL.to_owned(), service.name.to_string()),
            (
                RUN_ENVIRONMENT_LABEL.to_owned(),
                service.environment_id.to_string(),
            ),
        ]),
    }
}

// a run cannot outlive the birdd that streamed its output, so any of its environments' runs found
// at startup are left over; another birdd on the same podman keeps its own
pub(crate) async fn remove_leftover_runs(state: &AppState) {
    let leftovers = match state.podman.list_containers(RUN_LABEL).await {
        Ok(containers) => containers,
        Err(err) => {
            tracing::warn!(error = %err, "could not list one-off containers");
            return;
        }
    };
    let ours: HashSet<String> = match state.db.call(|store| store.list_all_environments()).await {
        Ok(environments) => environments
            .into_iter()
            .map(|env| env.id.to_string())
            .collect(),
        Err(err) => {
            tracing::warn!(error = %err, "could not list environments");
            return;
        }
    };
    let leftovers = leftovers.into_iter().filter(|container| {
        container
            .labels
            .get(RUN_ENVIRONMENT_LABEL)
            .is_some_and(|environment| ours.contains(environment))
    });
    for container in leftovers {
        tracing::info!(container = %container.name, "removing leftover one-off container");
        if let Err(err) = state.podman.remove_container(&container.id).await {
            tracing::warn!(container = %container.name, error = %err, "could not remove one-off container");
        }
    }
}
