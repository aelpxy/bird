use std::fmt;

use bird_core::UserId;
use rusqlite::params;

use crate::error::expect_changed;
use crate::{Result, Store};

// what checking a sign-in needs; kept apart from `User` and redacted when printed
#[derive(Clone, Default, PartialEq, Eq)]
pub struct LoginSecrets {
    pub password_hash: Option<String>,
    pub totp_secret: Option<String>,
    pub totp_pending: Option<String>,
}

impl fmt::Debug for LoginSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LoginSecrets")
            .field(
                "password_hash",
                &self.password_hash.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "totp_secret",
                &self.totp_secret.as_ref().map(|_| "<redacted>"),
            )
            .field(
                "totp_pending",
                &self.totp_pending.as_ref().map(|_| "<redacted>"),
            )
            .finish()
    }
}

impl Store {
    pub fn login_secrets(&self, user_id: UserId) -> Result<LoginSecrets> {
        Ok(self
            .query_one(
                "SELECT password_hash, totp_secret, totp_pending FROM users WHERE id = ?1",
                [user_id.to_string()],
                |row| {
                    Ok(LoginSecrets {
                        password_hash: row.get(0)?,
                        totp_secret: row.get(1)?,
                        totp_pending: row.get(2)?,
                    })
                },
            )?
            .unwrap_or_default())
    }

    pub fn set_password_hash(&self, user_id: UserId, hash: &str) -> Result<()> {
        let changed = self.execute(
            "UPDATE users SET password_hash = ?2 WHERE id = ?1",
            params![user_id.to_string(), hash],
        )?;
        expect_changed(changed, "user")
    }

    pub fn set_totp_pending(&self, user_id: UserId, secret: &str) -> Result<()> {
        let changed = self.execute(
            "UPDATE users SET totp_pending = ?2 WHERE id = ?1",
            params![user_id.to_string(), secret],
        )?;
        expect_changed(changed, "user")
    }

    // the pending secret becomes the one sign-ins check, with fresh recovery codes
    pub fn enable_totp(&mut self, user_id: UserId, recovery_hashes: &[String]) -> Result<()> {
        self.transaction(|store| {
            let changed = store.execute(
                "UPDATE users SET totp_secret = totp_pending, totp_pending = NULL,
                 totp_last_step = NULL WHERE id = ?1 AND totp_pending IS NOT NULL",
                [user_id.to_string()],
            )?;
            expect_changed(changed, "pending two-factor setup")?;
            store.execute(
                "DELETE FROM recovery_codes WHERE user_id = ?1",
                [user_id.to_string()],
            )?;
            for hash in recovery_hashes {
                store.execute(
                    "INSERT INTO recovery_codes (user_id, hash) VALUES (?1, ?2)",
                    params![user_id.to_string(), hash],
                )?;
            }
            Ok(())
        })
    }

    pub fn disable_totp(&mut self, user_id: UserId) -> Result<()> {
        self.transaction(|store| {
            store.execute(
                "UPDATE users SET totp_secret = NULL, totp_pending = NULL, totp_last_step = NULL
                 WHERE id = ?1",
                [user_id.to_string()],
            )?;
            store.execute(
                "DELETE FROM recovery_codes WHERE user_id = ?1",
                [user_id.to_string()],
            )?;
            Ok(())
        })
    }

    // true when no code was accepted for this or a later step, so each code works once
    pub fn accept_totp_step(&self, user_id: UserId, step: i64) -> Result<bool> {
        let changed = self.execute(
            "UPDATE users SET totp_last_step = ?2
             WHERE id = ?1 AND (totp_last_step IS NULL OR totp_last_step < ?2)",
            params![user_id.to_string(), step],
        )?;
        Ok(changed == 1)
    }

    // true when the code existed, which also uses it up
    pub fn use_recovery_code(&self, user_id: UserId, hash: &str) -> Result<bool> {
        let changed = self.execute(
            "DELETE FROM recovery_codes WHERE user_id = ?1 AND hash = ?2",
            params![user_id.to_string(), hash],
        )?;
        Ok(changed == 1)
    }

    pub fn recovery_codes_left(&self, user_id: UserId) -> Result<usize> {
        let left: i64 = self
            .query_one(
                "SELECT COUNT(*) FROM recovery_codes WHERE user_id = ?1",
                [user_id.to_string()],
                |row| row.get(0),
            )?
            .unwrap_or(0);
        Ok(usize::try_from(left).unwrap_or(0))
    }
}

#[cfg(test)]
mod tests {
    use bird_core::UserRole;

    use crate::Error;
    use crate::testing::{name, setup};

    #[test]
    fn keeps_secrets_off_users_and_uses_codes_once() {
        let (mut store, _) = setup();
        let ada = store.create_user(&name("ada"), UserRole::Member).unwrap();
        assert!(!ada.has_password && !ada.two_factor);
        store.set_password_hash(ada.id, "$argon2id$x").unwrap();
        assert!(matches!(
            store.enable_totp(ada.id, &[]).unwrap_err(),
            Error::NotFound("pending two-factor setup")
        ));
        store.set_totp_pending(ada.id, "JBSWY3DPEHPK3PXP").unwrap();
        store
            .enable_totp(ada.id, &["h1".to_owned(), "h2".to_owned()])
            .unwrap();

        let user = store.user(ada.id).unwrap().unwrap();
        assert!(user.has_password && user.two_factor);
        assert!(!format!("{user:?}").contains("argon2"));
        let secrets = store.login_secrets(ada.id).unwrap();
        assert_eq!(secrets.totp_secret.as_deref(), Some("JBSWY3DPEHPK3PXP"));
        assert_eq!(secrets.totp_pending, None);
        assert!(!format!("{secrets:?}").contains("JBSWY3DP"));

        assert!(store.accept_totp_step(ada.id, 10).unwrap());
        assert!(!store.accept_totp_step(ada.id, 10).unwrap());
        assert!(!store.accept_totp_step(ada.id, 9).unwrap());
        assert!(store.accept_totp_step(ada.id, 11).unwrap());

        assert!(store.use_recovery_code(ada.id, "h1").unwrap());
        assert!(!store.use_recovery_code(ada.id, "h1").unwrap());
        assert_eq!(store.recovery_codes_left(ada.id).unwrap(), 1);

        store.disable_totp(ada.id).unwrap();
        assert!(!store.user(ada.id).unwrap().unwrap().two_factor);
        assert_eq!(store.recovery_codes_left(ada.id).unwrap(), 0);
    }
}
