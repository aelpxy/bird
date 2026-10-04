use bird_core::{Name, User, UserId, UserRole};
use rusqlite::{Row, params};

use crate::error::{expect_changed, write_error};
use crate::rows::parse;
use crate::store::now;
use crate::{Result, Store};

const COLUMNS: &str = "id, name, role, created_at";

impl Store {
    pub fn create_user(&self, name: &Name, role: UserRole) -> Result<User> {
        let user = User {
            id: UserId::generate(),
            name: name.clone(),
            role,
            created_at: now(),
        };
        self.execute(
            "INSERT INTO users (id, name, role, created_at) VALUES (?1, ?2, ?3, ?4)",
            params![
                user.id.to_string(),
                user.name.as_str(),
                role.as_str(),
                user.created_at
            ],
        )
        .map_err(write_error("user"))?;
        Ok(user)
    }

    pub fn user_by_name(&self, name: &Name) -> Result<Option<User>> {
        self.query_one(
            &format!("SELECT {COLUMNS} FROM users WHERE name = ?1"),
            [name.as_str()],
            user,
        )
    }

    pub fn list_users(&self) -> Result<Vec<User>> {
        self.query_all(
            &format!("SELECT {COLUMNS} FROM users ORDER BY name"),
            [],
            user,
        )
    }

    // takes the user's tokens with it
    pub fn delete_user(&self, id: UserId) -> Result<()> {
        let changed = self.execute("DELETE FROM users WHERE id = ?1", [id.to_string()])?;
        expect_changed(changed, "user")
    }
}

pub(crate) fn user(row: &Row<'_>) -> rusqlite::Result<User> {
    Ok(User {
        id: parse(row, 0)?,
        name: parse(row, 1)?,
        role: parse(row, 2)?,
        created_at: row.get(3)?,
    })
}

#[cfg(test)]
mod tests {
    use bird_core::UserRole;

    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn creates_lists_and_deletes_users() {
        let (store, _) = setup();
        let ada = store.create_user(&name("ada"), UserRole::Admin).unwrap();
        store.create_user(&name("bob"), UserRole::Member).unwrap();
        assert!(matches!(
            store
                .create_user(&name("ada"), UserRole::Member)
                .unwrap_err(),
            Error::AlreadyExists("user")
        ));
        assert_eq!(store.user_by_name(&name("ada")).unwrap(), Some(ada.clone()));
        let names: Vec<_> = store
            .list_users()
            .unwrap()
            .into_iter()
            .map(|user| (user.name.to_string(), user.role))
            .collect();
        assert_eq!(
            names,
            [
                ("ada".to_owned(), UserRole::Admin),
                ("bob".to_owned(), UserRole::Member)
            ]
        );
        store.delete_user(ada.id).unwrap();
        assert!(store.user_by_name(&name("ada")).unwrap().is_none());
        assert!(matches!(
            store.delete_user(ada.id).unwrap_err(),
            Error::NotFound("user")
        ));
    }
}
