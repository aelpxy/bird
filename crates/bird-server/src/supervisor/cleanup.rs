use std::collections::{BTreeMap, HashSet};

use bird_core::{MachineId, MachineState};

use crate::deploy::{destroy_container, set_state};
use crate::labels;
use crate::state::AppState;

pub(super) async fn retire_machines(state: &AppState) {
    let machines = match state.db.call(|store| store.list_retirable_machines()).await {
        Ok(machines) => machines,
        Err(err) => {
            tracing::warn!(error = %err, "could not list machines to retire");
            return;
        }
    };
    for machine in machines {
        if let Some(container_id) = &machine.container_id {
            destroy_container(state, container_id).await;
        }
        set_state(state, machine.id, MachineState::Destroyed).await;
        tracing::info!(machine = %machine.id, "machine retired");
    }
}

pub(super) async fn remove_orphans(state: &AppState) {
    let tracked = match state
        .db
        .call(|store| store.list_tracked_machine_ids())
        .await
    {
        Ok(ids) => ids.into_iter().collect::<HashSet<_>>(),
        Err(err) => {
            tracing::warn!(error = %err, "could not list tracked machines");
            return;
        }
    };
    let environments = match state.db.call(|store| store.list_all_environments()).await {
        Ok(environments) => environments,
        Err(err) => {
            tracing::warn!(error = %err, "could not list environments");
            return;
        }
    };
    // only this birdd's environments, so another birdd on the same podman keeps its machines
    let mut containers = Vec::new();
    for environment in environments {
        let filter = labels::environment_filter(environment.id);
        match state.podman.list_containers(&filter).await {
            Ok(found) => containers.extend(found),
            Err(err) => {
                tracing::warn!(error = %err, "could not list containers");
                return;
            }
        }
    }
    for container in containers {
        if is_orphan(&container.labels, &tracked) {
            tracing::warn!(container = %container.name, "removing orphaned container");
            destroy_container(state, &container.id).await;
        }
    }
}

fn is_orphan(labels: &BTreeMap<String, String>, tracked: &HashSet<MachineId>) -> bool {
    labels
        .get(labels::MACHINE)
        .and_then(|raw| raw.parse::<MachineId>().ok())
        .is_none_or(|id| !tracked.contains(&id))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_orphans() {
        let known = MachineId::generate();
        let tracked = HashSet::from([known]);
        let with = |value: String| BTreeMap::from([(labels::MACHINE.to_owned(), value)]);

        assert!(!is_orphan(&with(known.to_string()), &tracked));
        assert!(is_orphan(
            &with(MachineId::generate().to_string()),
            &tracked
        ));
        assert!(is_orphan(&with("garbage".to_owned()), &tracked));
        assert!(is_orphan(&BTreeMap::new(), &tracked));
    }
}
