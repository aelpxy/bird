use bird_core::{
    Deployment, DeploymentStatus, ImageRef, Machine, MachineId, MachineState, ServiceState,
};
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
            "SELECT id, service_id, image, port, status, created_at, command FROM deployments
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

    // every image bird deployed, and whether it is live or among the last few distinct images a
    // service ran; redeploys and failed deploys do not push older images out
    pub fn list_deployed_images(&self, keep_recent: u32) -> Result<Vec<(ImageRef, bool)>> {
        self.query_all(
            "WITH used AS (
                 SELECT service_id, image,
                     MAX(CASE WHEN status != ?5 THEN id END) AS last_ran,
                     MAX(status IN (?2, ?3, ?4)) AS live
                 FROM deployments GROUP BY service_id, image
             ), ranked AS (
                 SELECT image, live, last_ran, ROW_NUMBER() OVER (
                     PARTITION BY service_id ORDER BY last_ran DESC
                 ) AS recency
                 FROM used
             )
             SELECT image, MAX((last_ran IS NOT NULL AND recency <= ?1) OR live)
             FROM ranked GROUP BY image",
            params![
                keep_recent,
                DeploymentStatus::Active.as_str(),
                DeploymentStatus::Deploying.as_str(),
                DeploymentStatus::Pending.as_str(),
                DeploymentStatus::Failed.as_str()
            ],
            rows::image_use,
        )
    }

    // stopped machines of a stopped service are kept on purpose, any other stop was a deploy cut short
    pub fn fail_interrupted(&mut self) -> Result<Interrupted> {
        self.transaction(|store| {
            let machines = store.execute(
                "UPDATE machines SET state = ?1 WHERE state IN (?2, ?3, ?4)
                 AND NOT (state = ?4 AND deployment_id IN (
                     SELECT d.id FROM deployments d JOIN services s ON s.id = d.service_id
                     WHERE s.state = ?5 AND d.status = ?6))",
                params![
                    MachineState::Failed.as_str(),
                    MachineState::Created.as_str(),
                    MachineState::Starting.as_str(),
                    MachineState::Stopped.as_str(),
                    ServiceState::Stopped.as_str(),
                    DeploymentStatus::Active.as_str()
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
    fn keeps_images_of_live_and_recent_deployments() {
        let (mut store, service) = setup();
        let deploy_image = |store: &mut crate::Store, image: &str| {
            store
                .update_service(service.id, &image.parse().unwrap(), service.port)
                .unwrap();
            let current = store.service(service.id).unwrap().unwrap();
            let deployment = store.create_deployment(&current).unwrap();
            store.activate_deployment(deployment.id).unwrap();
        };
        for tag in ["app:1", "app:2", "app:3", "app:4", "app:4", "app:4"] {
            deploy_image(&mut store, tag);
        }
        store
            .update_service(service.id, &"app:5".parse().unwrap(), service.port)
            .unwrap();
        let current = store.service(service.id).unwrap().unwrap();
        let failed = store.create_deployment(&current).unwrap();
        store
            .set_deployment_status(failed.id, DeploymentStatus::Failed)
            .unwrap();
        let mut images: Vec<(String, bool)> = store
            .list_deployed_images(2)
            .unwrap()
            .into_iter()
            .map(|(image, keep)| (image.to_string(), keep))
            .collect();
        images.sort();
        assert_eq!(
            images,
            [
                ("app:1".to_owned(), false),
                ("app:2".to_owned(), false),
                ("app:3".to_owned(), true),
                ("app:4".to_owned(), true),
                ("app:5".to_owned(), false),
            ]
        );
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

    #[test]
    fn keeps_machines_of_stopped_services() {
        let (mut store, service) = setup();
        let deployment = store.create_deployment(&service).unwrap();
        store.activate_deployment(deployment.id).unwrap();
        let stopped = store.create_machine(deployment.id).unwrap();
        store
            .set_machine_state(stopped.id, MachineState::Stopped)
            .unwrap();
        store
            .set_service_state(service.id, ServiceState::Stopped)
            .unwrap();
        assert_eq!(store.fail_interrupted().unwrap().machines, 0);

        store
            .set_service_state(service.id, ServiceState::Running)
            .unwrap();
        assert_eq!(store.fail_interrupted().unwrap().machines, 1);
    }
}
