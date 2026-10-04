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

    pub fn list_projects(&self) -> Result<Vec<Project>> {
        self.query_all(
            "SELECT id, name, created_at FROM projects ORDER BY name",
            [],
            rows::project,
        )
    }

    // takes its environments along, so the caller first makes sure they are empty
    pub fn delete_project(&self, id: ProjectId) -> Result<()> {
        self.execute("DELETE FROM projects WHERE id = ?1", [id.to_string()])?;
        Ok(())
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
    fn lists_and_deletes_with_environments() {
        let (store, _) = setup();
        let shop = store.create_project(&name("shop")).unwrap();
        let env = store
            .create_environment(shop.id, &name("production"), Some("bird_shop_production"))
            .unwrap();
        let names: Vec<_> = store
            .list_projects()
            .unwrap()
            .into_iter()
            .map(|project| project.name.to_string())
            .collect();
        assert_eq!(names, ["default", "shop"]);
        store.delete_project(shop.id).unwrap();
        assert!(store.project_by_name(&name("shop")).unwrap().is_none());
        assert!(store.environment(env.id).unwrap().is_none());
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
