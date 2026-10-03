use bird_core::{Registry, RegistryHost};
use rusqlite::params;

use crate::error::expect_changed;
use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn put_registry(&self, registry: &Registry) -> Result<()> {
        self.execute(
            "INSERT INTO registries (host, username, password, insecure, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT (host) DO UPDATE SET username = excluded.username,
                 password = excluded.password, insecure = excluded.insecure",
            params![
                registry.host.as_str(),
                registry.username,
                registry.password,
                registry.insecure,
                now()
            ],
        )?;
        Ok(())
    }

    pub fn registry(&self, host: &str) -> Result<Option<Registry>> {
        self.query_one(
            "SELECT host, username, password, insecure FROM registries WHERE host = ?1",
            [host],
            rows::registry,
        )
    }

    pub fn list_registries(&self) -> Result<Vec<Registry>> {
        self.query_all(
            "SELECT host, username, password, insecure FROM registries ORDER BY host",
            [],
            rows::registry,
        )
    }

    pub fn remove_registry(&self, host: &RegistryHost) -> Result<()> {
        let changed = self.execute("DELETE FROM registries WHERE host = ?1", [host.as_str()])?;
        expect_changed(changed, "registry")
    }
}

#[cfg(test)]
mod tests {
    use bird_core::Registry;

    use crate::Error;
    use crate::testing::setup;

    fn registry(password: &str) -> Registry {
        Registry {
            host: "ghcr.io".parse().unwrap(),
            username: "bird".to_owned(),
            password: password.to_owned(),
            insecure: false,
        }
    }

    #[test]
    fn upserts_finds_and_removes() {
        let (store, _) = setup();
        store.put_registry(&registry("one")).unwrap();
        store.put_registry(&registry("two")).unwrap();
        assert_eq!(store.registry("ghcr.io").unwrap(), Some(registry("two")));
        assert_eq!(store.list_registries().unwrap().len(), 1);
        assert_eq!(store.registry("docker.io").unwrap(), None);
        store.remove_registry(&"ghcr.io".parse().unwrap()).unwrap();
        assert!(matches!(
            store
                .remove_registry(&"ghcr.io".parse().unwrap())
                .unwrap_err(),
            Error::NotFound("registry")
        ));
    }
}
