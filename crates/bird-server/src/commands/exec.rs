use std::time::Duration;

use bird_core::{Command, EnvironmentId, Machine, MachineState, Name};
use bytes::Bytes;
use tokio::sync::mpsc;

use super::forward;
use crate::state::AppState;
use crate::{Error, Result};

const EXEC_TIMEOUT: Duration = Duration::from_hours(1);

// a full machine id, or its end as `bird status` and `bird logs` show it
pub(crate) async fn pick_machine(
    state: &AppState,
    environment: EnvironmentId,
    name: &Name,
    wanted: Option<&str>,
) -> Result<String> {
    let service = state.service(environment, name).await?;
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
    choose(name, machines, wanted)
}

fn choose(name: &Name, machines: Vec<Machine>, wanted: Option<&str>) -> Result<String> {
    let running: Vec<_> = machines
        .into_iter()
        .filter(|m| m.state == MachineState::Running)
        .filter_map(|m| Some((m.id.to_string(), m.container_id?)))
        .collect();
    let Some(wanted) = wanted else {
        return running
            .into_iter()
            .next()
            .map(|(_, container)| container)
            .ok_or_else(|| Error::NoMachines(name.clone()));
    };
    let mut matching = running
        .into_iter()
        .filter(|(id, _)| !wanted.is_empty() && id.ends_with(wanted));
    match (matching.next(), matching.next()) {
        (Some((_, container)), None) => Ok(container),
        (Some(_), Some(_)) => Err(Error::AmbiguousMachine(wanted.to_owned())),
        (None, _) => Err(Error::MachineNotFound {
            service: name.clone(),
            machine: wanted.to_owned(),
        }),
    }
}

// podman cannot stop an exec session, so a disconnect stops reading and leaves the command running
pub(crate) async fn exec(
    state: &AppState,
    container: &str,
    command: &Command,
    events: &mpsc::Sender<Bytes>,
) -> Result<Option<i32>> {
    let session = async {
        let mut session = state.podman.exec(container, command.args()).await?;
        if !forward(&mut session.output, events).await? {
            return Ok(None);
        }
        Ok(Some(state.podman.exec_exit_code(&session).await?))
    };
    tokio::time::timeout(EXEC_TIMEOUT, session)
        .await
        .unwrap_or_else(|_| {
            Err(Error::CommandFailed(format!(
                "stopped reading after {} minutes",
                EXEC_TIMEOUT.as_secs() / 60
            )))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn machine(id: &str, state: MachineState, container: Option<&str>) -> Machine {
        Machine {
            id: id.parse().unwrap(),
            deployment_id: "01a10467-92a5-7040-8b88-e1572362b0c3".parse().unwrap(),
            container_id: container.map(str::to_owned),
            address: None,
            state,
            created_at: 0,
            updated_at: 0,
        }
    }

    fn machines() -> Vec<Machine> {
        vec![
            machine(
                "01a10467-0000-7000-8000-00000000aa22",
                MachineState::Stopped,
                Some("old"),
            ),
            machine(
                "01a10467-0000-7000-8000-00000000bb11",
                MachineState::Running,
                Some("first"),
            ),
            machine(
                "01a10467-0000-7000-8000-00000000cc11",
                MachineState::Running,
                Some("second"),
            ),
            machine(
                "01a10467-0000-7000-8000-00000000dd22",
                MachineState::Starting,
                None,
            ),
        ]
    }

    #[test]
    fn picks_running_machines_by_the_end_of_their_id() {
        let name: Name = "web".parse().unwrap();
        assert_eq!(choose(&name, machines(), None).unwrap(), "first");
        assert_eq!(choose(&name, machines(), Some("cc11")).unwrap(), "second");
        let full = "01a10467-0000-7000-8000-00000000bb11";
        assert_eq!(choose(&name, machines(), Some(full)).unwrap(), "first");
    }

    #[test]
    fn rejects_ambiguous_stopped_or_unknown_machines() {
        let name: Name = "web".parse().unwrap();
        assert!(matches!(
            choose(&name, machines(), Some("11")),
            Err(Error::AmbiguousMachine(_))
        ));
        for missing in ["aa22", "dd22", "zzzz", ""] {
            assert!(
                matches!(
                    choose(&name, machines(), Some(missing)),
                    Err(Error::MachineNotFound { .. })
                ),
                "{missing}"
            );
        }
        assert!(matches!(
            choose(&name, Vec::new(), None),
            Err(Error::NoMachines(_))
        ));
    }
}
