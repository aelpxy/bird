use bird_core::{Deployment, DeploymentId, DeploymentStatus, Service, ServiceId};
use rusqlite::{OptionalExtension, params};

use crate::error::{expect_changed, write_error};
use crate::store::now;
use crate::{Error, Result, Store, rows};

impl Store {
    pub fn create_deployment(&self, service: &Service) -> Result<Deployment> {
        let deployment = Deployment {
            id: DeploymentId::generate(),
            service_id: service.id,
            image: service.image.clone(),
            port: service.port,
            status: DeploymentStatus::Pending,
            created_at: now(),
        };
        self.execute(
            "INSERT INTO deployments (id, service_id, image, port, status, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                deployment.id.to_string(),
                service.id.to_string(),
                deployment.image.as_str(),
                deployment.port.get(),
                deployment.status.as_str(),
                deployment.created_at
            ],
        )
        .map_err(write_error("deployment"))?;
        Ok(deployment)
    }

    pub fn deployment(&self, id: DeploymentId) -> Result<Option<Deployment>> {
        self.query_one(
            "SELECT id, service_id, image, port, status, created_at FROM deployments WHERE id = ?1",
            [id.to_string()],
            rows::deployment,
        )
    }

    pub fn active_deployment(&self, service_id: ServiceId) -> Result<Option<Deployment>> {
        self.query_one(
            "SELECT id, service_id, image, port, status, created_at FROM deployments
             WHERE service_id = ?1 AND status = ?2",
            params![service_id.to_string(), DeploymentStatus::Active.as_str()],
            rows::deployment,
        )
    }

    pub fn list_deployments(&self, service_id: ServiceId) -> Result<Vec<Deployment>> {
        self.query_all(
            "SELECT id, service_id, image, port, status, created_at FROM deployments
             WHERE service_id = ?1 ORDER BY created_at DESC, id DESC",
            [service_id.to_string()],
            rows::deployment,
        )
    }

    pub fn set_deployment_status(&self, id: DeploymentId, status: DeploymentStatus) -> Result<()> {
        let changed = self
            .execute(
                "UPDATE deployments SET status = ?2 WHERE id = ?1",
                params![id.to_string(), status.as_str()],
            )
            .map_err(write_error("active deployment"))?;
        expect_changed(changed, "deployment")
    }

    pub fn activate_deployment(&mut self, id: DeploymentId) -> Result<()> {
        let tx = self.conn.transaction()?;
        let service_id: String = tx
            .query_row(
                "SELECT service_id FROM deployments WHERE id = ?1",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(Error::NotFound("deployment"))?;
        tx.execute(
            "UPDATE deployments SET status = ?3 WHERE service_id = ?1 AND status = ?2",
            params![
                service_id,
                DeploymentStatus::Active.as_str(),
                DeploymentStatus::Superseded.as_str()
            ],
        )?;
        tx.execute(
            "UPDATE deployments SET status = ?2 WHERE id = ?1",
            params![id.to_string(), DeploymentStatus::Active.as_str()],
        )?;
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{DeploymentStatus, Port};

    use crate::Error;
    use crate::testing::setup;

    #[test]
    fn activation_supersedes_previous() {
        let (mut store, service) = setup();
        let first = store.create_deployment(&service).unwrap();
        let second = store.create_deployment(&service).unwrap();

        store.activate_deployment(first.id).unwrap();
        assert_eq!(
            store.active_deployment(service.id).unwrap().unwrap().id,
            first.id
        );

        store.activate_deployment(second.id).unwrap();
        assert_eq!(
            store.active_deployment(service.id).unwrap().unwrap().id,
            second.id
        );
        assert_eq!(
            store.deployment(first.id).unwrap().unwrap().status,
            DeploymentStatus::Superseded
        );
        assert_eq!(
            store
                .list_deployments(service.id)
                .unwrap()
                .iter()
                .map(|d| d.id)
                .collect::<Vec<_>>(),
            vec![second.id, first.id]
        );
    }

    #[test]
    fn only_one_active_deployment() {
        let (mut store, service) = setup();
        let first = store.create_deployment(&service).unwrap();
        let second = store.create_deployment(&service).unwrap();
        store.activate_deployment(second.id).unwrap();
        assert!(matches!(
            store
                .set_deployment_status(first.id, DeploymentStatus::Active)
                .unwrap_err(),
            Error::AlreadyExists(_)
        ));
    }

    #[test]
    fn snapshots_service_config() {
        let (store, service) = setup();
        let deployment = store.create_deployment(&service).unwrap();
        store
            .update_service(
                service.id,
                &"nginx:1.27".parse().unwrap(),
                Port::try_from(8080).unwrap(),
            )
            .unwrap();
        let stored = store.deployment(deployment.id).unwrap().unwrap();
        assert_eq!(stored.image.as_str(), "nginx:latest");
        assert_eq!(stored.port.get(), 80);
    }
}
