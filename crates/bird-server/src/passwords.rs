use std::sync::OnceLock;

use argon2::Argon2;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};

use crate::{Error, Result, secrets};

pub(crate) const MIN_CHARS: usize = 12;
pub(crate) const MAX_CHARS: usize = 256;

pub(crate) fn check_policy(password: &str) -> Result<()> {
    let chars = password.chars().count();
    if (MIN_CHARS..=MAX_CHARS).contains(&chars) {
        Ok(())
    } else {
        Err(Error::WeakPassword {
            min: MIN_CHARS,
            max: MAX_CHARS,
        })
    }
}

// argon2id is slow on purpose, so it runs off the async threads
pub(crate) async fn hash(password: String) -> Result<String> {
    tokio::task::spawn_blocking(move || hash_now(&password))
        .await
        .map_err(|err| Error::PasswordHash(err.to_string()))?
}

// with no hash, a stand-in is checked, so a missing user or password answers as slowly as a wrong one
pub(crate) async fn verify(password: String, hash: Option<String>) -> bool {
    tokio::task::spawn_blocking(move || {
        let Some(stand_in) = stand_in() else {
            return false;
        };
        let matched = Argon2::default()
            .verify_password(password.as_bytes(), hash.as_deref().unwrap_or(stand_in))
            .is_ok();
        matched && hash.is_some()
    })
    .await
    .unwrap_or(false)
}

fn hash_now(password: &str) -> Result<String> {
    Argon2::default()
        .hash_password(password.as_bytes())
        .map(|hashed| hashed.to_string())
        .map_err(|err| Error::PasswordHash(err.to_string()))
}

fn stand_in() -> Option<&'static str> {
    static STAND_IN: OnceLock<Option<String>> = OnceLock::new();
    STAND_IN
        .get_or_init(|| {
            let random = secrets::generate(32).ok()?;
            hash_now(&random).ok()
        })
        .as_deref()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn hashes_verify_and_policy_holds() {
        let hashed = hash("correct horse battery".to_owned()).await.unwrap();
        assert!(hashed.starts_with("$argon2id$"));
        assert!(verify("correct horse battery".to_owned(), Some(hashed.clone())).await);
        assert!(!verify("wrong horse battery".to_owned(), Some(hashed)).await);
        assert!(!verify("anything at all".to_owned(), None).await);
        assert!(check_policy("short").is_err());
        assert!(check_policy("twelve chars").is_ok());
        assert!(check_policy(&"x".repeat(MAX_CHARS + 1)).is_err());
    }
}
