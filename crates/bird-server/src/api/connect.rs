use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::extract::{Query, Request, State};
use axum::http::header::{CONNECTION, UPGRADE};
use axum::http::{HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use bird_api::{ErrorBody, TCP_UPGRADE};
use bird_core::{Machine, MachineState, Name, Service};
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;
use serde::Deserialize;
use tokio::net::TcpStream;
use tokio::sync::OwnedSemaphorePermit;

use super::scope::ServiceScope;
use crate::state::AppState;
use crate::{Error, Result};

pub(crate) const MAX_TUNNELS: usize = 256;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub(crate) struct ConnectQuery {
    /// Machine id, or the end of one; left out, the first running machine
    machine: Option<String>,
}

/// Open a TCP connection to the service's port, for `bird connect`
#[utoipa::path(get, operation_id = "connect_service", path = "/v1/projects/{project}/environments/{environment}/services/{name}/connect", tag = "commands", params(("project" = String, Path, description = "Project name"), ("environment" = String, Path, description = "Environment name"), ("name" = String, Path, description = "Service name"), ConnectQuery), responses((status = 101, description = "Switched to bird-tcp: the connection now carries the bytes of a TCP connection to the service's port"), (status = 401, description = "Missing or invalid API token", body = ErrorBody), (status = 404, description = "Service or machine not found, or nothing running", body = ErrorBody), (status = 426, description = "The request did not ask to upgrade to bird-tcp", body = ErrorBody), (status = 502, description = "The machine did not accept the connection", body = ErrorBody), (status = 503, description = "Too many tunnels are open", body = ErrorBody)))]
pub(crate) async fn connect(
    State(state): State<AppState>,
    ServiceScope { environment, name }: ServiceScope,
    Query(query): Query<ConnectQuery>,
    request: Request,
) -> Result<Response> {
    let (mut parts, _) = request.into_parts();
    let asked = parts.headers.get(UPGRADE).map(HeaderValue::as_bytes);
    if !asked.is_some_and(|protocol| protocol.eq_ignore_ascii_case(TCP_UPGRADE.as_bytes())) {
        return Err(Error::TunnelUpgradeRequired);
    }
    let upgrade = parts
        .extensions
        .remove::<OnUpgrade>()
        .ok_or(Error::TunnelUpgradeRequired)?;
    let permit = Arc::clone(&state.tunnels)
        .try_acquire_owned()
        .map_err(|_| Error::TooManyTunnels)?;
    let service = state.service(environment, &name).await?;
    let address = machine_address(&state, &service, query.machine.as_deref()).await?;
    // connected before switching, so a refused connection is an error the client can show
    let upstream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
        Ok(Ok(stream)) => stream,
        Ok(Err(err)) => return Err(unreachable(&service, err.to_string())),
        Err(_) => return Err(unreachable(&service, "timed out".to_owned())),
    };
    tracing::info!(service = %name, %address, "tunnel opened");
    tokio::spawn(relay(state, name, upgrade, upstream, permit));
    Ok((
        StatusCode::SWITCHING_PROTOCOLS,
        [(CONNECTION, "upgrade"), (UPGRADE, TCP_UPGRADE)],
    )
        .into_response())
}

fn unreachable(service: &Service, reason: String) -> Error {
    Error::Unreachable {
        service: service.name.clone(),
        port: service.port,
        reason,
    }
}

async fn machine_address(
    state: &AppState,
    service: &Service,
    wanted: Option<&str>,
) -> Result<SocketAddr> {
    let service_id = service.id;
    let machines = state
        .db
        .call(move |store| {
            let Some(deployment) = store.active_deployment(service_id)? else {
                return Ok(Vec::new());
            };
            store.list_machines(deployment.id)
        })
        .await?;
    choose(&service.name, &machines, wanted)
}

fn choose(name: &Name, machines: &[Machine], wanted: Option<&str>) -> Result<SocketAddr> {
    let mut running = machines
        .iter()
        .filter(|machine| machine.state == MachineState::Running)
        .filter(|machine| {
            wanted
                .is_none_or(|wanted| !wanted.is_empty() && machine.id.to_string().ends_with(wanted))
        })
        .filter_map(|machine| machine.address);
    let first = running.next();
    match (first, wanted, running.next()) {
        (Some(_), Some(wanted), Some(_)) => Err(Error::AmbiguousMachine(wanted.to_owned())),
        (Some(address), _, _) => Ok(address),
        (None, Some(wanted), _) => Err(Error::MachineNotFound {
            service: name.clone(),
            machine: wanted.to_owned(),
        }),
        (None, None, _) => Err(Error::NoMachines(name.clone())),
    }
}

// lasts as long as either side keeps it open, or until birdd shuts down
async fn relay(
    state: AppState,
    name: Name,
    upgrade: OnUpgrade,
    mut upstream: TcpStream,
    permit: OwnedSemaphorePermit,
) {
    let _permit = permit;
    let mut client = match upgrade.await {
        Ok(upgraded) => TokioIo::new(upgraded),
        Err(err) => {
            tracing::debug!(service = %name, error = %err, "tunnel upgrade did not complete");
            return;
        }
    };
    tokio::select! {
        copied = tokio::io::copy_bidirectional(&mut client, &mut upstream) => match copied {
            Ok((sent, received)) => tracing::info!(service = %name, sent, received, "tunnel closed"),
            Err(err) => tracing::debug!(service = %name, error = %err, "tunnel closed with an error"),
        },
        () = state.shutdown.wait() => tracing::debug!(service = %name, "tunnel closed for shutdown"),
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{DeploymentId, MachineId};

    use super::*;

    fn machine(state: MachineState, port: Option<u16>) -> Machine {
        Machine {
            id: MachineId::generate(),
            deployment_id: DeploymentId::generate(),
            container_id: None,
            state,
            address: port.map(|port| SocketAddr::from(([127, 0, 0, 1], port))),
            created_at: 0,
            updated_at: 0,
        }
    }

    #[test]
    fn picks_a_running_machine_with_an_address() {
        let name: Name = "db".parse().unwrap();
        let machines = [
            machine(MachineState::Stopped, Some(1)),
            machine(MachineState::Running, None),
            machine(MachineState::Running, Some(3)),
        ];
        assert_eq!(choose(&name, &machines, None).unwrap().port(), 3);
        let id = machines[2].id.to_string();
        let end = &id[id.len() - 6..];
        assert_eq!(choose(&name, &machines, Some(end)).unwrap().port(), 3);
        assert!(matches!(
            choose(&name, &machines, Some("zzzzzz")),
            Err(Error::MachineNotFound { .. })
        ));
        assert!(matches!(
            choose(&name, &[], None),
            Err(Error::NoMachines(_))
        ));
    }
}
