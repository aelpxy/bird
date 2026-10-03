use bird_core::{Deployment, DeploymentStatus, Machine, MachineId, MachineState};
use rusqlite::params;

use crate::{Result, Store, rows};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Interrupted {
    pub machines: usize,
    pub deployments: usize,
}

impl Store {
    pub fn list_active_deployments(&self) -> Result<Vec<Deployment>> {
        self.query_all(
            "SELECT id, service_id, image, port, status, created_at FROM deployments
             WHERE status = ?1 ORDER BY id",
            [DeploymentStatus::Active.as_str()],
            rows::deployment,
        )
    }

    pub fn list_tracked_machine_ids(&self) -> Result<Vec<MachineId>> {
        self.query_all(
            "SELECT id FROM machines WHERE state != ?1 ORDER BY id",
            [MachineState::Destroyed.as_str()],
            rows::machine_id,
        )
    }

    pub fn list_retirable_machines(&self) -> Result<Vec<Machine>> {
        self.query_all(
            "SELECT m.id, m.deployment_id, m.container_id, m.address, m.state, m.created_at,
                    m.updated_at
             FROM machines m JOIN deployments d ON d.id = m.deployment_id
             WHERE m.state != ?1 AND (m.state IN (?2, ?5) OR d.status IN (?3, ?4))
             ORDER BY m.id",
            params![
                MachineState::Destroyed.as_str(),
                MachineState::Failed.as_str(),
                DeploymentStatus::Superseded.as_str(),
                DeploymentStatus::Failed.as_str(),
                MachineState::Stopping.as_str()
            ],
            rows::machine,
        )
    }

    pub fn fail_interrupted(&mut self) -> Result<Interrupted> {
        self.transaction(|store| {
            let machines = store.execute(
                "UPDATE machines SET state = ?1 WHERE state IN (?2, ?3, ?4)",
                params![
                    MachineState::Failed.as_str(),
                    MachineState::Created.as_str(),
                    MachineState::Starting.as_str(),
                    MachineState::Stopped.as_str()
                ],
            )?;
            let deployments = store.execute(
                "UPDATE deployments SET status = ?1 WHERE status IN (?2, ?3)",
                params![
                    DeploymentStatus::Failed.as_str(),
                    DeploymentStatus::Pending.as_str(),
                    DeploymentStatus::Deploying.as_str()
                ],
            )?;
            Ok(Interrupted {
                machines,
                deployments,
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{DeploymentStatus, MachineState};

    use super::*;
    use crate::testing::setup;

    #[test]
    fn finds_active_deployments() {
        let (mut store, service) = setup();
        let old = store.create_deployment(&service).unwrap();
        let new = store.create_deployment(&service).unwrap();
        store.activate_deployment(old.id).unwrap();
        store.activate_deployment(new.id).unwrap();
        let active: Vec<_> = store
            .list_active_deployments()
            .unwrap()
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert_eq!(active, vec![new.id]);
    }

    #[test]
    fn retirable_covers_failed_and_superseded_machines() {
        let (mut store, service) = setup();
        let old = store.create_deployment(&service).unwrap();
        let old_machine = store.create_machine(old.id).unwrap();
        store.activate_deployment(old.id).unwrap();

        let new = store.create_deployment(&service).unwrap();
        let healthy = store.create_machine(new.id).unwrap();
        let broken = store.create_machine(new.id).unwrap();
        store
            .set_machine_state(healthy.id, MachineState::Running)
            .unwrap();
        store
            .set_machine_state(broken.id, MachineState::Failed)
            .unwrap();
        store.activate_deployment(new.id).unwrap();

        let retirable: Vec<_> = store
            .list_retirable_machines()
            .unwrap()
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(retirable, vec![old_machine.id, broken.id]);

        store
            .set_machine_state(old_machine.id, MachineState::Destroyed)
            .unwrap();
        let tracked = store.list_tracked_machine_ids().unwrap();
        assert_eq!(tracked, vec![healthy.id, broken.id]);
    }

    #[test]
    fn fails_interrupted_work() {
        let (mut store, service) = setup();
        let deployment = store.create_deployment(&service).unwrap();
        store
            .set_deployment_status(deployment.id, DeploymentStatus::Deploying)
            .unwrap();
        let starting = store.create_machine(deployment.id).unwrap();
        store
            .set_machine_state(starting.id, MachineState::Starting)
            .unwrap();
        store.create_machine(deployment.id).unwrap();
        let paused = store.create_machine(deployment.id).unwrap();
        store
            .set_machine_state(paused.id, MachineState::Stopped)
            .unwrap();

        assert_eq!(
            store.fail_interrupted().unwrap(),
            Interrupted {
                machines: 3,
                deployments: 1
            }
        );
        assert_eq!(
            store.deployment(deployment.id).unwrap().unwrap().status,
            DeploymentStatus::Failed
        );
        assert!(
            store
                .list_machines(deployment.id)
                .unwrap()
                .iter()
                .all(|m| m.state == MachineState::Failed)
        );
    }
}
