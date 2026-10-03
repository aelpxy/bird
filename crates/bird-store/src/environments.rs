use bird_core::{Environment, EnvironmentId, Name, ProjectId};
use rusqlite::params;

use crate::error::write_error;
use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn create_environment(&self, project_id: ProjectId, name: &Name) -> Result<Environment> {
        let environment = Environment {
            id: EnvironmentId::generate(),
            project_id,
            name: name.clone(),
            created_at: now(),
        };
        self.execute(
            "INSERT INTO environments (id, project_id, name, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                environment.id.to_string(),
                project_id.to_string(),
                environment.name.as_str(),
                environment.created_at
            ],
        )
        .map_err(write_error("environment"))?;
        Ok(environment)
    }

    pub fn environment_by_name(
        &self,
        project_id: ProjectId,
        name: &Name,
    ) -> Result<Option<Environment>> {
        self.query_one(
            "SELECT id, project_id, name, created_at FROM environments
             WHERE project_id = ?1 AND name = ?2",
            params![project_id.to_string(), name.as_str()],
            rows::environment,
        )
    }
}

#[cfg(test)]
mod tests {
    use bird_core::ProjectId;

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
    }

    #[test]
    fn rejects_missing_project() {
        let (store, _) = setup();
        assert!(matches!(
            store
                .create_environment(ProjectId::generate(), &name("staging"))
                .unwrap_err(),
            Error::ParentNotFound("environment")
        ));
    }
}
