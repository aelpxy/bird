use std::net::SocketAddr;

use bird_core::{DeploymentId, Machine, MachineId, MachineState, ServiceId};
use rusqlite::params;

use crate::error::{expect_changed, write_error};
use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn create_machine(&self, deployment_id: DeploymentId) -> Result<Machine> {
        let created_at = now();
        let machine = Machine {
            id: MachineId::generate(),
            deployment_id,
            container_id: None,
            address: None,
            state: MachineState::Created,
            created_at,
            updated_at: created_at,
        };
        self.execute(
            "INSERT INTO machines (id, deployment_id, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![
                machine.id.to_string(),
                deployment_id.to_string(),
                machine.state.as_str(),
                created_at
            ],
        )
        .map_err(write_error("machine"))?;
        Ok(machine)
    }

    pub fn set_machine_state(&self, id: MachineId, state: MachineState) -> Result<()> {
        let changed = self.execute(
            "UPDATE machines SET state = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.to_string(), state.as_str(), now()],
        )?;
        expect_changed(changed, "machine")
    }

    pub fn set_machine_container(&self, id: MachineId, container_id: &str) -> Result<()> {
        let changed = self.execute(
            "UPDATE machines SET container_id = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.to_string(), container_id, now()],
        )?;
        expect_changed(changed, "machine")
    }

    pub fn set_machine_address(&self, id: MachineId, address: SocketAddr) -> Result<()> {
        let changed = self.execute(
            "UPDATE machines SET address = ?2, updated_at = ?3 WHERE id = ?1",
            params![id.to_string(), address.to_string(), now()],
        )?;
        expect_changed(changed, "machine")
    }

    pub fn list_machines(&self, deployment_id: DeploymentId) -> Result<Vec<Machine>> {
        self.query_all(
            "SELECT id, deployment_id, container_id, address, state, created_at, updated_at
             FROM machines WHERE deployment_id = ?1 ORDER BY id",
            [deployment_id.to_string()],
            rows::machine,
        )
    }

    pub fn list_service_machines(&self, service_id: ServiceId) -> Result<Vec<Machine>> {
        self.query_all(
            "SELECT m.id, m.deployment_id, m.container_id, m.address, m.state, m.created_at,
                    m.updated_at
             FROM machines m JOIN deployments d ON d.id = m.deployment_id
             WHERE d.service_id = ?1 ORDER BY m.id",
            [service_id.to_string()],
            rows::machine,
        )
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;

    use bird_core::{DeploymentId, MachineId, MachineState};

    use crate::Error;
    use crate::testing::setup;

    #[test]
    fn tracks_state_container_and_address() {
        let (store, service) = setup();
        let deployment = store.create_deployment(&service).unwrap();
        let machine = store.create_machine(deployment.id).unwrap();
        let address: SocketAddr = "127.0.0.1:45765".parse().unwrap();
        store.set_machine_container(machine.id, "abc123").unwrap();
        store.set_machine_address(machine.id, address).unwrap();
        store
            .set_machine_state(machine.id, MachineState::Running)
            .unwrap();

        let machines = store.list_machines(deployment.id).unwrap();
        assert_eq!(machines.len(), 1);
        assert_eq!(machines[0].container_id.as_deref(), Some("abc123"));
        assert_eq!(machines[0].address, Some(address));
        assert_eq!(machines[0].state, MachineState::Running);
        assert_eq!(store.list_service_machines(service.id).unwrap(), machines);
    }

    #[test]
    fn rejects_unknown_ids() {
        let (store, _) = setup();
        assert!(matches!(
            store.create_machine(DeploymentId::generate()).unwrap_err(),
            Error::ParentNotFound("machine")
        ));
        assert!(matches!(
            store
                .set_machine_state(MachineId::generate(), MachineState::Running)
                .unwrap_err(),
            Error::NotFound("machine")
        ));
    }
}
