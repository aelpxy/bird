use bird_core::{Environment, EnvironmentId, Name, ProjectId};
use rusqlite::params;

use crate::error::write_error;
use crate::store::now;
use crate::{Result, Store, rows};

const COLUMNS: &str = "id, project_id, name, network, created_at";

impl Store {
    pub fn create_environment(
        &self,
        project_id: ProjectId,
        name: &Name,
        network: Option<&str>,
    ) -> Result<Environment> {
        let environment = Environment {
            id: EnvironmentId::generate(),
            project_id,
            name: name.clone(),
            network: network.map(str::to_owned),
            created_at: now(),
        };
        self.execute(
            "INSERT INTO environments (id, project_id, name, network, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                environment.id.to_string(),
                project_id.to_string(),
                environment.name.as_str(),
                environment.network,
                environment.created_at
            ],
        )
        .map_err(write_error("environment"))?;
        Ok(environment)
    }

    pub fn environment(&self, id: EnvironmentId) -> Result<Option<Environment>> {
        self.query_one(
            &format!("SELECT {COLUMNS} FROM environments WHERE id = ?1"),
            [id.to_string()],
            rows::environment,
        )
    }

    pub fn environment_by_name(
        &self,
        project_id: ProjectId,
        name: &Name,
    ) -> Result<Option<Environment>> {
        self.query_one(
            &format!("SELECT {COLUMNS} FROM environments WHERE project_id = ?1 AND name = ?2"),
            params![project_id.to_string(), name.as_str()],
            rows::environment,
        )
    }

    pub fn list_environments(&self, project_id: ProjectId) -> Result<Vec<Environment>> {
        self.query_all(
            &format!("SELECT {COLUMNS} FROM environments WHERE project_id = ?1 ORDER BY name"),
            [project_id.to_string()],
            rows::environment,
        )
    }

    pub fn list_all_environments(&self) -> Result<Vec<Environment>> {
        self.query_all(
            &format!("SELECT {COLUMNS} FROM environments ORDER BY name"),
            [],
            rows::environment,
        )
    }

    // also forgets its backups, so the caller first makes sure none are left
    pub fn delete_environment(&self, id: EnvironmentId) -> Result<()> {
        self.execute("DELETE FROM environments WHERE id = ?1", [id.to_string()])?;
        Ok(())
    }

    // what still lives in an environment: its services, and backups that outlive their service
    pub fn environment_contents(&self, id: EnvironmentId) -> Result<(Vec<Name>, Vec<Name>)> {
        let services = self.query_all(
            "SELECT name FROM services WHERE environment_id = ?1 ORDER BY name",
            [id.to_string()],
            |row| rows::parse(row, 0),
        )?;
        let backed_up = self.query_all(
            "SELECT DISTINCT service_name FROM backups WHERE environment_id = ?1 ORDER BY service_name",
            [id.to_string()],
            |row| rows::parse(row, 0),
        )?;
        Ok((services, backed_up))
    }
}

#[cfg(test)]
mod tests {
    use bird_core::{Backup, BackupId, BackupTrigger, ProjectId};

    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn finds_by_name() {
        let (store, service) = setup();
        let project = store.project_by_name(&name("default")).unwrap().unwrap();
        let env = store
            .environment_by_name(project.id, &name("production"))
            .unwrap()
            .unwrap();
        assert_eq!(env.id, service.environment_id);
        assert_eq!(store.environment(env.id).unwrap(), Some(env));
    }

    #[test]
    fn rejects_missing_project() {
        let (store, _) = setup();
        assert!(matches!(
            store
                .create_environment(ProjectId::generate(), &name("staging"), None)
                .unwrap_err(),
            Error::ParentNotFound("environment")
        ));
    }

    #[test]
    fn keeps_networks_apart_and_lists_what_is_left() {
        let (mut store, service) = setup();
        let project = store.project_by_name(&name("default")).unwrap().unwrap();
        let staging = store
            .create_environment(project.id, &name("staging"), Some("bird_default_staging"))
            .unwrap();
        assert!(matches!(
            store
                .create_environment(project.id, &name("qa"), Some("bird_default_staging"))
                .unwrap_err(),
            Error::AlreadyExists("environment")
        ));
        let names: Vec<_> = store
            .list_environments(project.id)
            .unwrap()
            .into_iter()
            .map(|env| (env.name.to_string(), env.network))
            .collect();
        assert_eq!(
            names,
            [
                ("production".to_owned(), None),
                (
                    "staging".to_owned(),
                    Some("bird_default_staging".to_owned())
                )
            ]
        );

        assert_eq!(
            store.environment_contents(staging.id).unwrap(),
            (Vec::new(), Vec::new())
        );
        store
            .create_backup(Backup {
                id: BackupId::generate(),
                environment_id: staging.id,
                service: name("db"),
                trigger: BackupTrigger::Manual,
                storage: "local".to_owned(),
                volumes: Vec::new(),
                created_at: 0,
            })
            .unwrap();
        assert_eq!(
            store.environment_contents(staging.id).unwrap(),
            (Vec::new(), vec![name("db")])
        );
        assert_eq!(
            store
                .environment_contents(service.environment_id)
                .unwrap()
                .0,
            std::slice::from_ref(&service.name)
        );
        store.delete_environment(staging.id).unwrap();
        assert_eq!(store.environment(staging.id).unwrap(), None);
    }
}
