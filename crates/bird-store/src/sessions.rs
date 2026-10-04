use bird_core::{Session, SessionId, User, UserId};
use rusqlite::{Row, params};

use crate::error::{expect_changed, write_error};
use crate::rows::parse;
use crate::store::now;
use crate::users::{user, user_columns};
use crate::{Result, Store};

const COLUMNS: &str =
    "s.id, s.user_id, s.created_at, s.last_used_at, s.expires_at, s.address, s.agent";
// use is recorded at most this often, so busy clients do not write on every call
const USE_RESOLUTION_SECS: i64 = 60;

pub struct NewSession<'a> {
    pub user_id: UserId,
    pub hash: &'a str,
    pub expires_at: i64,
    pub address: Option<&'a str>,
    pub agent: Option<&'a str>,
}

impl Store {
    pub fn create_session(&self, new: &NewSession<'_>) -> Result<Session> {
        let at = now();
        let session = Session {
            id: SessionId::generate(),
            user_id: new.user_id,
            created_at: at,
            last_used_at: at,
            expires_at: new.expires_at,
            address: new.address.map(str::to_owned),
            agent: new.agent.map(str::to_owned),
        };
        self.execute(
            "INSERT INTO sessions (id, user_id, hash, created_at, last_used_at, expires_at, address, agent)
             VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6, ?7)",
            params![
                session.id.to_string(),
                session.user_id.to_string(),
                new.hash,
                at,
                session.expires_at,
                session.address,
                session.agent
            ],
        )
        .map_err(write_error("session"))?;
        Ok(session)
    }

    // the session's user, unless it expired or sat unused longer than `idle_secs`
    pub fn authenticate_session(
        &self,
        hash: &str,
        idle_secs: i64,
    ) -> Result<Option<(User, Session)>> {
        let at = now();
        self.query_one(
            &format!(
                "SELECT {}, {COLUMNS} FROM sessions s JOIN users u ON u.id = s.user_id
                 WHERE s.hash = ?1 AND s.expires_at > ?2 AND s.last_used_at > ?3",
                user_columns("u")
            ),
            params![hash, at, at - idle_secs],
            |row| Ok((user(row)?, session_at(row, 6)?)),
        )
    }

    pub fn record_session_use(&self, id: SessionId) -> Result<()> {
        let at = now();
        self.execute(
            "UPDATE sessions SET last_used_at = ?2 WHERE id = ?1 AND last_used_at <= ?3",
            params![id.to_string(), at, at - USE_RESOLUTION_SECS],
        )?;
        Ok(())
    }

    pub fn list_sessions(&self, user_id: UserId) -> Result<Vec<Session>> {
        self.query_all(
            &format!(
                "SELECT {COLUMNS} FROM sessions s WHERE s.user_id = ?1 AND s.expires_at > ?2
                 ORDER BY s.last_used_at DESC"
            ),
            params![user_id.to_string(), now()],
            |row| session_at(row, 0),
        )
    }

    pub fn delete_session(&self, user_id: UserId, id: SessionId) -> Result<()> {
        let changed = self.execute(
            "DELETE FROM sessions WHERE user_id = ?1 AND id = ?2",
            params![user_id.to_string(), id.to_string()],
        )?;
        expect_changed(changed, "session")
    }

    // signs the user out everywhere, except in the session `keep` when given
    pub fn delete_sessions(&self, user_id: UserId, keep: Option<SessionId>) -> Result<usize> {
        let keep = keep.map(|id| id.to_string()).unwrap_or_default();
        Ok(self.execute(
            "DELETE FROM sessions WHERE user_id = ?1 AND id != ?2",
            params![user_id.to_string(), keep],
        )?)
    }
}

fn session_at(row: &Row<'_>, at: usize) -> rusqlite::Result<Session> {
    Ok(Session {
        id: parse(row, at)?,
        user_id: parse(row, at + 1)?,
        created_at: row.get(at + 2)?,
        last_used_at: row.get(at + 3)?,
        expires_at: row.get(at + 4)?,
        address: row.get(at + 5)?,
        agent: row.get(at + 6)?,
    })
}

#[cfg(test)]
mod tests {
    use bird_core::UserRole;

    use super::*;
    use crate::testing::{name, setup};

    #[test]
    fn sessions_end_on_expiry_idleness_or_sign_out() {
        let (store, _) = setup();
        let ada = store.create_user(&name("ada"), UserRole::Member).unwrap();
        let new = |hash, expires_at| NewSession {
            user_id: ada.id,
            hash,
            expires_at,
            address: Some("127.0.0.1"),
            agent: Some("bird-cli"),
        };
        let laptop = store.create_session(&new("h1", now() + 3600)).unwrap();
        let phone = store.create_session(&new("h2", now() + 3600)).unwrap();
        store.create_session(&new("h3", now() - 1)).unwrap();

        let (user, found) = store.authenticate_session("h1", 600).unwrap().unwrap();
        assert_eq!((user.id, found.id), (ada.id, laptop.id));
        assert!(store.authenticate_session("h3", 600).unwrap().is_none());
        // used just now, so any idle window in the past rejects it
        assert!(store.authenticate_session("h1", -1).unwrap().is_none());
        assert_eq!(store.list_sessions(ada.id).unwrap().len(), 2);

        assert_eq!(store.delete_sessions(ada.id, Some(laptop.id)).unwrap(), 2);
        assert!(store.authenticate_session("h2", 600).unwrap().is_none());
        assert!(store.authenticate_session("h1", 600).unwrap().is_some());
        assert!(store.delete_session(ada.id, phone.id).is_err());
        store.delete_session(ada.id, laptop.id).unwrap();
        assert_eq!(store.list_sessions(ada.id).unwrap(), Vec::new());
    }
}
