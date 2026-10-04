use bird_core::{ApiToken, Name, TokenId, User, UserId};
use rusqlite::{Row, params};

use crate::error::{expect_changed, write_error};
use crate::rows::parse;
use crate::store::now;
use crate::users::{user, user_columns};
use crate::{Result, Store};

const COLUMNS: &str = "id, user_id, name, prefix, created_at, last_used_at, expires_at";
// a request records use at most this often, so busy clients do not write on every call
const USE_RESOLUTION_SECS: i64 = 60;

pub struct NewToken<'a> {
    pub user_id: UserId,
    pub name: &'a Name,
    pub hash: &'a str,
    pub prefix: &'a str,
    pub expires_at: Option<i64>,
}

impl Store {
    pub fn create_token(&self, new: &NewToken<'_>) -> Result<ApiToken> {
        let token = ApiToken {
            id: TokenId::generate(),
            user_id: new.user_id,
            name: new.name.clone(),
            prefix: new.prefix.to_owned(),
            created_at: now(),
            last_used_at: None,
            expires_at: new.expires_at,
        };
        self.execute(
            "INSERT INTO api_tokens (id, user_id, name, hash, prefix, created_at, expires_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                token.id.to_string(),
                token.user_id.to_string(),
                token.name.as_str(),
                new.hash,
                token.prefix,
                token.created_at,
                token.expires_at
            ],
        )
        .map_err(write_error("token"))?;
        Ok(token)
    }

    pub fn list_tokens(&self, user_id: UserId) -> Result<Vec<ApiToken>> {
        self.query_all(
            &format!("SELECT {COLUMNS} FROM api_tokens WHERE user_id = ?1 ORDER BY name"),
            [user_id.to_string()],
            token,
        )
    }

    pub fn delete_token(&self, user_id: UserId, name: &Name) -> Result<()> {
        let changed = self.execute(
            "DELETE FROM api_tokens WHERE user_id = ?1 AND name = ?2",
            params![user_id.to_string(), name.as_str()],
        )?;
        expect_changed(changed, "token")
    }

    // the user a token belongs to, if it exists and has not expired
    pub fn authenticate(&self, hash: &str) -> Result<Option<(User, ApiToken)>> {
        self.query_one(
            &format!(
                "SELECT {},
                        t.id, t.user_id, t.name, t.prefix, t.created_at, t.last_used_at, t.expires_at
                 FROM api_tokens t JOIN users u ON u.id = t.user_id
                 WHERE t.hash = ?1 AND (t.expires_at IS NULL OR t.expires_at > ?2)",
                user_columns("u")
            ),
            params![hash, now()],
            |row| Ok((user(row)?, token_at(row, 6)?)),
        )
    }

    pub fn record_token_use(&self, id: TokenId) -> Result<()> {
        let at = now();
        self.execute(
            "UPDATE api_tokens SET last_used_at = ?2
             WHERE id = ?1 AND (last_used_at IS NULL OR last_used_at <= ?3)",
            params![id.to_string(), at, at - USE_RESOLUTION_SECS],
        )?;
        Ok(())
    }
}

fn token(row: &Row<'_>) -> rusqlite::Result<ApiToken> {
    token_at(row, 0)
}

fn token_at(row: &Row<'_>, at: usize) -> rusqlite::Result<ApiToken> {
    Ok(ApiToken {
        id: parse(row, at)?,
        user_id: parse(row, at + 1)?,
        name: parse(row, at + 2)?,
        prefix: row.get(at + 3)?,
        created_at: row.get(at + 4)?,
        last_used_at: row.get(at + 5)?,
        expires_at: row.get(at + 6)?,
    })
}

#[cfg(test)]
mod tests {
    use bird_core::UserRole;

    use super::*;
    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn authenticates_live_tokens_only() {
        let (store, _) = setup();
        let ada = store.create_user(&name("ada"), UserRole::Admin).unwrap();
        let laptop = name("laptop");
        let new = |name, hash, expires_at| NewToken {
            user_id: ada.id,
            name,
            hash,
            prefix: "bird_abc",
            expires_at,
        };
        let token = store.create_token(&new(&laptop, "h1", None)).unwrap();
        assert!(matches!(
            store.create_token(&new(&laptop, "h2", None)).unwrap_err(),
            Error::AlreadyExists("token")
        ));
        let old = name("old");
        store
            .create_token(&new(&old, "h3", Some(now() - 1)))
            .unwrap();

        let (user, found) = store.authenticate("h1").unwrap().unwrap();
        assert_eq!((user.name, found.id), (name("ada"), token.id));
        assert!(store.authenticate("h3").unwrap().is_none());
        assert!(store.authenticate("nope").unwrap().is_none());

        store.record_token_use(token.id).unwrap();
        let listed = store.list_tokens(ada.id).unwrap();
        assert_eq!(listed.len(), 2);
        assert!(
            listed
                .iter()
                .any(|t| t.name == laptop && t.last_used_at.is_some())
        );

        store.delete_token(ada.id, &laptop).unwrap();
        assert!(store.authenticate("h1").unwrap().is_none());
        let ci = name("ci");
        store.create_token(&new(&ci, "h4", None)).unwrap();
        store.delete_user(ada.id).unwrap();
        assert!(store.authenticate("h4").unwrap().is_none());
    }
}
