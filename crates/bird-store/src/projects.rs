use bird_core::{Name, Project, ProjectId};
use rusqlite::params;

use crate::error::write_error;
use crate::store::now;
use crate::{Result, Store, rows};

impl Store {
    pub fn create_project(&self, name: &Name) -> Result<Project> {
        let project = Project {
            id: ProjectId::generate(),
            name: name.clone(),
            created_at: now(),
        };
        self.execute(
            "INSERT INTO projects (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![
                project.id.to_string(),
                project.name.as_str(),
                project.created_at
            ],
        )
        .map_err(write_error("project"))?;
        Ok(project)
    }

    pub fn project_by_name(&self, name: &Name) -> Result<Option<Project>> {
        self.query_one(
            "SELECT id, name, created_at FROM projects WHERE name = ?1",
            [name.as_str()],
            rows::project,
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn finds_by_name() {
        let (store, _) = setup();
        assert!(store.project_by_name(&name("default")).unwrap().is_some());
        assert!(store.project_by_name(&name("missing")).unwrap().is_none());
    }

    #[test]
    fn rejects_duplicate() {
        let (store, _) = setup();
        assert!(matches!(
            store.create_project(&name("default")).unwrap_err(),
            Error::AlreadyExists("project")
        ));
    }
}
