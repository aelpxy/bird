use bird_core::{Domain, Hostname, ServiceId};
use rusqlite::params;

use crate::error::{expect_changed, write_error};
use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn add_domain(&self, service_id: ServiceId, hostname: &Hostname) -> Result<Domain> {
        let domain = Domain {
            hostname: hostname.clone(),
            service_id,
            created_at: now(),
        };
        self.execute(
            "INSERT INTO domains (hostname, service_id, created_at) VALUES (?1, ?2, ?3)",
            params![
                domain.hostname.as_str(),
                service_id.to_string(),
                domain.created_at
            ],
        )
        .map_err(write_error("domain"))?;
        Ok(domain)
    }

    pub fn remove_domain(&self, hostname: &Hostname) -> Result<()> {
        let changed = self.execute(
            "DELETE FROM domains WHERE hostname = ?1",
            [hostname.as_str()],
        )?;
        expect_changed(changed, "domain")
    }

    pub fn list_domains(&self, service_id: ServiceId) -> Result<Vec<Domain>> {
        self.query_all(
            "SELECT hostname, service_id, created_at FROM domains
             WHERE service_id = ?1 ORDER BY hostname",
            [service_id.to_string()],
            rows::domain,
        )
    }
}

#[cfg(test)]
mod tests {
    use bird_core::Hostname;

    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn globally_unique() {
        let (store, service) = setup();
        let other = store
            .create_service(
                service.environment_id,
                &name("api"),
                &service.image,
                service.port,
            )
            .unwrap();
        let host: Hostname = "web.localhost".parse().unwrap();
        store.add_domain(service.id, &host).unwrap();
        assert!(matches!(
            store.add_domain(other.id, &host).unwrap_err(),
            Error::AlreadyExists("domain")
        ));
    }

    #[test]
    fn add_list_remove() {
        let (store, service) = setup();
        let host: Hostname = "web.localhost".parse().unwrap();
        store.add_domain(service.id, &host).unwrap();
        assert_eq!(store.list_domains(service.id).unwrap().len(), 1);
        store.remove_domain(&host).unwrap();
        assert_eq!(store.list_domains(service.id).unwrap(), Vec::new());
        assert!(matches!(
            store.remove_domain(&host).unwrap_err(),
            Error::NotFound("domain")
        ));
    }
}
