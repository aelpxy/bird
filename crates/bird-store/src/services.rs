use bird_core::{EnvironmentId, ImageRef, Name, Port, Service, ServiceId};
use rusqlite::params;

use crate::error::{expect_changed, write_error};
use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn create_service(
        &self,
        environment_id: EnvironmentId,
        name: &Name,
        image: &ImageRef,
        port: Port,
    ) -> Result<Service> {
        let service = Service {
            id: ServiceId::generate(),
            environment_id,
            name: name.clone(),
            image: image.clone(),
            port,
            created_at: now(),
        };
        self.execute(
            "INSERT INTO services (id, environment_id, name, image, port, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                service.id.to_string(),
                environment_id.to_string(),
                service.name.as_str(),
                service.image.as_str(),
                port.get(),
                service.created_at
            ],
        )
        .map_err(write_error("service"))?;
        Ok(service)
    }

    pub fn update_service(&self, id: ServiceId, image: &ImageRef, port: Port) -> Result<()> {
        let changed = self.execute(
            "UPDATE services SET image = ?2, port = ?3 WHERE id = ?1",
            params![id.to_string(), image.as_str(), port.get()],
        )?;
        expect_changed(changed, "service")
    }

    pub fn service(&self, id: ServiceId) -> Result<Option<Service>> {
        self.query_one(
            "SELECT id, environment_id, name, image, port, created_at FROM services WHERE id = ?1",
            [id.to_string()],
            rows::service,
        )
    }

    pub fn service_by_name(
        &self,
        environment_id: EnvironmentId,
        name: &Name,
    ) -> Result<Option<Service>> {
        self.query_one(
            "SELECT id, environment_id, name, image, port, created_at FROM services
             WHERE environment_id = ?1 AND name = ?2",
            params![environment_id.to_string(), name.as_str()],
            rows::service,
        )
    }

    pub fn list_services(&self, environment_id: EnvironmentId) -> Result<Vec<Service>> {
        self.query_all(
            "SELECT id, environment_id, name, image, port, created_at FROM services
             WHERE environment_id = ?1 ORDER BY name",
            [environment_id.to_string()],
            rows::service,
        )
    }

    pub fn delete_service(&self, id: ServiceId) -> Result<()> {
        let changed = self.execute("DELETE FROM services WHERE id = ?1", [id.to_string()])?;
        expect_changed(changed, "service")
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{EnvironmentId, Port};

    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn finds_and_lists() {
        let (store, service) = setup();
        assert_eq!(store.service(service.id).unwrap(), Some(service.clone()));
        assert_eq!(
            store
                .service_by_name(service.environment_id, &name("web"))
                .unwrap(),
            Some(service.clone())
        );
        assert_eq!(
            store.list_services(service.environment_id).unwrap(),
            vec![service]
        );
    }

    #[test]
    fn rejects_duplicates_and_missing_environment() {
        let (store, service) = setup();
        assert!(matches!(
            store
                .create_service(
                    service.environment_id,
                    &name("web"),
                    &service.image,
                    service.port
                )
                .unwrap_err(),
            Error::AlreadyExists("service")
        ));
        assert!(matches!(
            store
                .create_service(
                    EnvironmentId::generate(),
                    &name("api"),
                    &service.image,
                    service.port
                )
                .unwrap_err(),
            Error::ParentNotFound("service")
        ));
    }

    #[test]
    fn updates_config() {
        let (store, service) = setup();
        store
            .update_service(
                service.id,
                &"nginx:1.27".parse().unwrap(),
                Port::try_from(8080).unwrap(),
            )
            .unwrap();
        let updated = store.service(service.id).unwrap().unwrap();
        assert_eq!(updated.image.as_str(), "nginx:1.27");
        assert_eq!(updated.port.get(), 8080);
    }

    #[test]
    fn delete_cascades() {
        let (store, service) = setup();
        let deployment = store.create_deployment(&service).unwrap();
        store.create_machine(deployment.id).unwrap();
        store
            .add_domain(service.id, &"web.localhost".parse().unwrap())
            .unwrap();
        store.delete_service(service.id).unwrap();
        assert!(store.deployment(deployment.id).unwrap().is_none());
        assert_eq!(store.list_machines(deployment.id).unwrap(), Vec::new());
        assert_eq!(store.list_domains(service.id).unwrap(), Vec::new());
        assert!(matches!(
            store.delete_service(service.id).unwrap_err(),
            Error::NotFound("service")
        ));
    }
}
