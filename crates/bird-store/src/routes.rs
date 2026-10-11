use std::net::SocketAddr;

use bird_core::{DeploymentStatus, Hostname, MachineId, MachineState, ProxySettings, ServiceId};
use rusqlite::params;

use crate::{Result, Store, rows};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RouteEntry {
    pub hostname: Hostname,
    pub address: Option<SocketAddr>,
    pub machine_id: Option<MachineId>,
    pub service_id: ServiceId,
    pub proxy: ProxySettings,
    pub redirect_to: Option<Hostname>,
}

impl Store {
    pub fn list_routes(&self) -> Result<Vec<RouteEntry>> {
        self.query_all(
            "SELECT d.hostname, m.address, d.service_id, s.proxy, d.redirect_to, m.id
             FROM domains d
             JOIN services s ON s.id = d.service_id
             LEFT JOIN deployments dep ON dep.service_id = d.service_id AND dep.status = ?1
             LEFT JOIN machines m ON m.deployment_id = dep.id AND m.state = ?2
                 AND m.address IS NOT NULL
             ORDER BY d.hostname, m.id",
            params![
                DeploymentStatus::Active.as_str(),
                MachineState::Running.as_str()
            ],
            rows::route,
        )
    }
}

#[cfg(test)]
mod tests {
    use bird_core::MachineState;

    use super::*;
    use crate::testing::setup;

    #[test]
    fn routes_only_running_machines_of_active_deployment() {
        let (mut store, service) = setup();
        let host: Hostname = "web.localhost".parse().unwrap();
        store.add_domain(service.id, &host).unwrap();
        assert_eq!(
            store.list_routes().unwrap(),
            vec![RouteEntry {
                hostname: host.clone(),
                address: None,
                machine_id: None,
                service_id: service.id,
                proxy: ProxySettings::default(),
                redirect_to: None,
            }]
        );

        let old = store.create_deployment(&service).unwrap();
        let old_machine = store.create_machine(old.id).unwrap();
        store
            .set_machine_address(old_machine.id, "127.0.0.1:1001".parse().unwrap())
            .unwrap();
        store
            .set_machine_state(old_machine.id, MachineState::Running)
            .unwrap();
        store.activate_deployment(old.id).unwrap();

        let new = store.create_deployment(&service).unwrap();
        let new_machine = store.create_machine(new.id).unwrap();
        store
            .set_machine_address(new_machine.id, "127.0.0.1:1002".parse().unwrap())
            .unwrap();
        store
            .set_machine_state(new_machine.id, MachineState::Running)
            .unwrap();
        assert_eq!(
            store.list_routes().unwrap()[0].address,
            Some("127.0.0.1:1001".parse().unwrap())
        );

        store.activate_deployment(new.id).unwrap();
        assert_eq!(
            store.list_routes().unwrap(),
            vec![RouteEntry {
                hostname: host,
                address: Some("127.0.0.1:1002".parse().unwrap()),
                machine_id: Some(new_machine.id),
                service_id: service.id,
                proxy: ProxySettings::default(),
                redirect_to: None,
            }]
        );
    }
}
