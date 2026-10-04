use bird_core::{
    Command, CpuLimit, EnvironmentId, HealthCheck, HealthTimeout, ImageRef, MemoryLimit, Name,
    Port, Replicas, Service, ServiceId, ServiceState,
};
use rusqlite::params;

use crate::error::{expect_changed, write_error};
use crate::store::now;
use crate::{Error, Result, Store, rows};

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
            replicas: Replicas::ONE,
            health: HealthCheck::Http,
            health_timeout: HealthTimeout::DEFAULT,
            state: ServiceState::Running,
            command: None,
            memory: MemoryLimit::DEFAULT,
            cpus: CpuLimit::DEFAULT,
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

    pub fn set_replicas(&self, id: ServiceId, replicas: Replicas) -> Result<()> {
        let changed = self.execute(
            "UPDATE services SET replicas = ?2 WHERE id = ?1",
            params![id.to_string(), replicas.get()],
        )?;
        expect_changed(changed, "service")
    }

    pub fn set_health(&self, id: ServiceId, health: &HealthCheck) -> Result<()> {
        let (kind, path) = health_columns(health);
        let changed = self.execute(
            "UPDATE services SET health = ?2, health_path = ?3 WHERE id = ?1",
            params![id.to_string(), kind, path],
        )?;
        expect_changed(changed, "service")
    }

    pub fn set_health_timeout(&self, id: ServiceId, timeout: HealthTimeout) -> Result<()> {
        let changed = self.execute(
            "UPDATE services SET health_timeout_secs = ?2 WHERE id = ?1",
            params![id.to_string(), timeout.secs()],
        )?;
        expect_changed(changed, "service")
    }

    pub fn set_command(&self, id: ServiceId, command: &Command) -> Result<()> {
        let encoded = serde_json::to_string(command.args()).map_err(Error::Encode)?;
        let changed = self.execute(
            "UPDATE services SET command = ?2 WHERE id = ?1",
            params![id.to_string(), encoded],
        )?;
        expect_changed(changed, "service")
    }

    pub fn set_resources(&self, id: ServiceId, memory: MemoryLimit, cpus: CpuLimit) -> Result<()> {
        let changed = self.execute(
            "UPDATE services SET memory_mb = ?2, cpu_millicores = ?3 WHERE id = ?1",
            params![id.to_string(), memory.mebibytes(), cpus.millicores()],
        )?;
        expect_changed(changed, "service")
    }

    // puts back what a deploy changed, so a failed deploy leaves no settings behind
    pub fn restore_settings(&self, service: &Service) -> Result<()> {
        let command = service
            .command
            .as_ref()
            .map(|command| serde_json::to_string(command.args()))
            .transpose()
            .map_err(Error::Encode)?;
        let (health, health_path) = health_columns(&service.health);
        let changed = self.execute(
            "UPDATE services SET image = ?2, port = ?3, health = ?4, health_path = ?5,
             health_timeout_secs = ?6, command = ?7, memory_mb = ?8, cpu_millicores = ?9,
             state = ?10
             WHERE id = ?1",
            params![
                service.id.to_string(),
                service.image.as_str(),
                service.port.get(),
                health,
                health_path,
                service.health_timeout.secs(),
                command,
                service.memory.mebibytes(),
                service.cpus.millicores(),
                service.state.as_str()
            ],
        )?;
        expect_changed(changed, "service")
    }

    pub fn service(&self, id: ServiceId) -> Result<Option<Service>> {
        self.query_one(
            "SELECT id, environment_id, name, image, port, created_at, replicas, health, command,
                    memory_mb, cpu_millicores, health_path, health_timeout_secs, state
             FROM services WHERE id = ?1",
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
            "SELECT id, environment_id, name, image, port, created_at, replicas, health, command,
                    memory_mb, cpu_millicores, health_path, health_timeout_secs, state
             FROM services
             WHERE environment_id = ?1 AND name = ?2",
            params![environment_id.to_string(), name.as_str()],
            rows::service,
        )
    }

    pub fn list_services(&self, environment_id: EnvironmentId) -> Result<Vec<Service>> {
        self.query_all(
            "SELECT id, environment_id, name, image, port, created_at, replicas, health, command,
                    memory_mb, cpu_millicores, health_path, health_timeout_secs, state
             FROM services
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

// the column's CHECK predates paths, so a path check is stored as http plus its path
fn health_columns(health: &HealthCheck) -> (&'static str, Option<&str>) {
    match health {
        HealthCheck::Http => ("http", None),
        HealthCheck::Path(path) => ("http", Some(path.as_str())),
        HealthCheck::Tcp => ("tcp", None),
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{EnvironmentId, HealthCheck, Port, Replicas};

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
    fn stores_replica_count() {
        let (store, service) = setup();
        assert_eq!(service.replicas, Replicas::ONE);
        let three = Replicas::try_from(3).unwrap();
        store.set_replicas(service.id, three).unwrap();
        assert_eq!(store.service(service.id).unwrap().unwrap().replicas, three);
    }

    #[test]
    fn stores_resource_limits() {
        let (store, service) = setup();
        assert_eq!(service.memory, bird_core::MemoryLimit::DEFAULT);
        let memory = "512m".parse().unwrap();
        let cpus = "0.5".parse().unwrap();
        store.set_resources(service.id, memory, cpus).unwrap();
        let stored = store.service(service.id).unwrap().unwrap();
        assert_eq!((stored.memory, stored.cpus), (memory, cpus));
    }

    #[test]
    fn stores_health_check() {
        let (store, service) = setup();
        assert_eq!(service.health, HealthCheck::Http);
        for check in [HealthCheck::Tcp, "/up".parse().unwrap(), HealthCheck::Http] {
            store.set_health(service.id, &check).unwrap();
            assert_eq!(store.service(service.id).unwrap().unwrap().health, check);
        }
        let timeout = "5m".parse().unwrap();
        store.set_health_timeout(service.id, timeout).unwrap();
        assert_eq!(
            store.service(service.id).unwrap().unwrap().health_timeout,
            timeout
        );
    }

    #[test]
    fn restores_settings() {
        let (store, service) = setup();
        let command = bird_core::Command::try_from(vec!["sh".to_owned()]).unwrap();
        store.set_command(service.id, &command).unwrap();
        store
            .set_health(service.id, &"/up".parse().unwrap())
            .unwrap();
        store
            .set_health_timeout(service.id, "90s".parse().unwrap())
            .unwrap();
        store
            .update_service(service.id, &"x:1".parse().unwrap(), service.port)
            .unwrap();
        store.restore_settings(&service).unwrap();
        assert_eq!(store.service(service.id).unwrap(), Some(service));
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
