use std::collections::BTreeMap;

use bird_core::{DeploymentId, EnvKey, ServiceId, Variable};
use rusqlite::params;

use crate::error::{expect_changed, write_error};
use crate::{Result, Store, rows};

impl Store {
    pub fn set_variable(&self, service_id: ServiceId, key: &EnvKey, value: &str) -> Result<()> {
        self.execute(
            "INSERT INTO variables (service_id, key, value) VALUES (?1, ?2, ?3)
             ON CONFLICT (service_id, key) DO UPDATE SET value = excluded.value",
            params![service_id.to_string(), key.as_str(), value],
        )
        .map_err(write_error("variable"))?;
        Ok(())
    }

    pub fn unset_variable(&self, service_id: ServiceId, key: &EnvKey) -> Result<()> {
        let changed = self.execute(
            "DELETE FROM variables WHERE service_id = ?1 AND key = ?2",
            params![service_id.to_string(), key.as_str()],
        )?;
        expect_changed(changed, "variable")
    }

    pub fn deployment_variables(
        &self,
        deployment_id: DeploymentId,
    ) -> Result<BTreeMap<EnvKey, String>> {
        let pairs = self.query_all(
            "SELECT key, value FROM deployment_variables WHERE deployment_id = ?1",
            [deployment_id.to_string()],
            rows::key_value,
        )?;
        Ok(pairs.into_iter().collect())
    }

    pub fn restore_variables(
        &mut self,
        service_id: ServiceId,
        deployment_id: DeploymentId,
    ) -> Result<()> {
        self.transaction(|store| {
            store.execute(
                "DELETE FROM variables WHERE service_id = ?1",
                [service_id.to_string()],
            )?;
            store.execute(
                "INSERT INTO variables (service_id, key, value)
                 SELECT ?1, key, value FROM deployment_variables WHERE deployment_id = ?2",
                params![service_id.to_string(), deployment_id.to_string()],
            )?;
            Ok(())
        })
    }

    pub fn list_variables(&self, service_id: ServiceId) -> Result<Vec<Variable>> {
        self.query_all(
            "SELECT service_id, key, value FROM variables WHERE service_id = ?1 ORDER BY key",
            [service_id.to_string()],
            rows::variable,
        )
    }
}

#[cfg(test)]
mod tests {
    use bird_core::EnvKey;

    use crate::Error;
    use crate::testing::setup;

    #[test]
    fn deployments_snapshot_variables() {
        let (mut store, service) = setup();
        let key: EnvKey = "MODE".parse().unwrap();
        store.set_variable(service.id, &key, "old").unwrap();
        let first = store.create_deployment(&service).unwrap();
        store.set_variable(service.id, &key, "new").unwrap();
        store
            .set_variable(service.id, &"EXTRA".parse().unwrap(), "1")
            .unwrap();
        let second = store.create_deployment(&service).unwrap();

        let snapshot = store.deployment_variables(first.id).unwrap();
        assert_eq!(snapshot.len(), 1);
        assert_eq!(snapshot.get(&key).map(String::as_str), Some("old"));
        assert_eq!(store.deployment_variables(second.id).unwrap().len(), 2);

        store.restore_variables(service.id, first.id).unwrap();
        let restored = store.list_variables(service.id).unwrap();
        assert_eq!(restored.len(), 1);
        assert_eq!(restored[0].value, "old");
    }

    #[test]
    fn upserts_and_unsets() {
        let (store, service) = setup();
        let key: EnvKey = "PORT".parse().unwrap();
        store.set_variable(service.id, &key, "80").unwrap();
        store.set_variable(service.id, &key, "8080").unwrap();
        let vars = store.list_variables(service.id).unwrap();
        assert_eq!(vars.len(), 1);
        assert_eq!(vars[0].value, "8080");
        store.unset_variable(service.id, &key).unwrap();
        assert_eq!(store.list_variables(service.id).unwrap(), Vec::new());
        assert!(matches!(
            store.unset_variable(service.id, &key).unwrap_err(),
            Error::NotFound("variable")
        ));
    }
}
