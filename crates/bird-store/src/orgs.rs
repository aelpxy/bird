use bird_core::{Name, Org, OrgId, OrgRole, User, UserId};
use rusqlite::{Row, params};

use crate::error::{expect_changed, write_error};
use crate::rows::parse;
use crate::store::now;
use crate::users::{user, user_columns};
use crate::{Result, Store};

const COLUMNS: &str = "id, name, created_at";

impl Store {
    pub fn create_org(&self, name: &Name) -> Result<Org> {
        let org = Org {
            id: OrgId::generate(),
            name: name.clone(),
            created_at: now(),
        };
        self.execute(
            "INSERT INTO orgs (id, name, created_at) VALUES (?1, ?2, ?3)",
            params![org.id.to_string(), org.name.as_str(), org.created_at],
        )
        .map_err(write_error("org"))?;
        Ok(org)
    }

    pub fn org(&self, id: OrgId) -> Result<Option<Org>> {
        self.query_one(
            &format!("SELECT {COLUMNS} FROM orgs WHERE id = ?1"),
            [id.to_string()],
            org,
        )
    }

    pub fn org_by_name(&self, name: &Name) -> Result<Option<Org>> {
        self.query_one(
            &format!("SELECT {COLUMNS} FROM orgs WHERE name = ?1"),
            [name.as_str()],
            org,
        )
    }

    pub fn list_orgs(&self) -> Result<Vec<Org>> {
        self.query_all(
            &format!("SELECT {COLUMNS} FROM orgs ORDER BY name"),
            [],
            org,
        )
    }

    // the caller first makes sure it owns no projects
    pub fn delete_org(&self, id: OrgId) -> Result<()> {
        let changed = self.execute("DELETE FROM orgs WHERE id = ?1", [id.to_string()])?;
        expect_changed(changed, "org")
    }

    // adds the user, or changes the role of one who already belongs
    pub fn set_member(&self, org_id: OrgId, user_id: UserId, role: OrgRole) -> Result<()> {
        self.execute(
            "INSERT INTO org_members (org_id, user_id, role) VALUES (?1, ?2, ?3)
             ON CONFLICT (org_id, user_id) DO UPDATE SET role = excluded.role",
            params![org_id.to_string(), user_id.to_string(), role.as_str()],
        )
        .map_err(write_error("org member"))?;
        Ok(())
    }

    pub fn remove_member(&self, org_id: OrgId, user_id: UserId) -> Result<()> {
        let changed = self.execute(
            "DELETE FROM org_members WHERE org_id = ?1 AND user_id = ?2",
            params![org_id.to_string(), user_id.to_string()],
        )?;
        expect_changed(changed, "org member")
    }

    pub fn membership(&self, org_id: OrgId, user_id: UserId) -> Result<Option<OrgRole>> {
        self.query_one(
            "SELECT role FROM org_members WHERE org_id = ?1 AND user_id = ?2",
            params![org_id.to_string(), user_id.to_string()],
            |row| parse(row, 0),
        )
    }

    pub fn list_members(&self, org_id: OrgId) -> Result<Vec<(User, OrgRole)>> {
        self.query_all(
            &format!(
                "SELECT {}, m.role
                 FROM org_members m JOIN users u ON u.id = m.user_id
                 WHERE m.org_id = ?1 ORDER BY u.name",
                user_columns("u")
            ),
            [org_id.to_string()],
            |row| Ok((user(row)?, parse(row, 6)?)),
        )
    }

    pub fn user_orgs(&self, user_id: UserId) -> Result<Vec<(Org, OrgRole)>> {
        self.query_all(
            "SELECT o.id, o.name, o.created_at, m.role
             FROM org_members m JOIN orgs o ON o.id = m.org_id
             WHERE m.user_id = ?1 ORDER BY o.name",
            [user_id.to_string()],
            |row| Ok((org(row)?, parse(row, 3)?)),
        )
    }
}

fn org(row: &Row<'_>) -> rusqlite::Result<Org> {
    Ok(Org {
        id: parse(row, 0)?,
        name: parse(row, 1)?,
        created_at: row.get(2)?,
    })
}

#[cfg(test)]
mod tests {
    use bird_core::{OrgRole, UserRole};

    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn manages_members_and_their_roles() {
        let (store, _) = setup();
        let org = store.org_by_name(&name("default")).unwrap().unwrap();
        let team = store.create_org(&name("team")).unwrap();
        let ada = store.create_user(&name("ada"), UserRole::Member).unwrap();
        assert!(matches!(
            store.create_org(&name("team")).unwrap_err(),
            Error::AlreadyExists("org")
        ));

        store.set_member(team.id, ada.id, OrgRole::Member).unwrap();
        store.set_member(team.id, ada.id, OrgRole::Owner).unwrap();
        assert_eq!(
            store.membership(team.id, ada.id).unwrap(),
            Some(OrgRole::Owner)
        );
        assert_eq!(store.membership(org.id, ada.id).unwrap(), None);
        let members: Vec<_> = store
            .list_members(team.id)
            .unwrap()
            .into_iter()
            .map(|(user, role)| (user.name, role))
            .collect();
        assert_eq!(members, [(name("ada"), OrgRole::Owner)]);
        assert_eq!(
            store.user_orgs(ada.id).unwrap(),
            [(team.clone(), OrgRole::Owner)]
        );

        store.remove_member(team.id, ada.id).unwrap();
        assert!(matches!(
            store.remove_member(team.id, ada.id).unwrap_err(),
            Error::NotFound("org member")
        ));
        store.set_member(team.id, ada.id, OrgRole::Admin).unwrap();
        store.delete_user(ada.id).unwrap();
        assert_eq!(store.list_members(team.id).unwrap(), Vec::new());
        store.delete_org(team.id).unwrap();
        assert_eq!(store.org(team.id).unwrap(), None);
    }
}
