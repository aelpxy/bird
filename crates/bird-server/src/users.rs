use std::fmt::Write as _;
use std::num::NonZeroU16;
use std::time::{SystemTime, UNIX_EPOCH};

use bird_core::{ApiToken, Name, User, UserRole};
use bird_store::NewToken;
use ring::digest;

use crate::state::AppState;
use crate::{Error, Result, secrets};

// recognisable in logs and secret scanners, like `ghp_` for github
const TOKEN_PREFIX: &str = "bird_";
const SECRET_CHARS: usize = 40;
// shown in listings so tokens can be told apart, too short to be of use to anyone else
const SHOWN_CHARS: usize = 12;
const SECONDS_PER_DAY: i64 = 86_400;
// the token a new user gets, to log in with
const FIRST_TOKEN: &str = "first";

pub(crate) struct Issued {
    pub(crate) token: ApiToken,
    pub(crate) secret: String,
}

#[must_use]
pub(crate) fn hash(secret: &str) -> String {
    digest::digest(&digest::SHA256, secret.as_bytes())
        .as_ref()
        .iter()
        .fold(String::new(), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

pub(crate) async fn find(state: &AppState, name: &Name) -> Result<User> {
    let lookup = name.clone();
    state
        .db
        .call(move |store| store.user_by_name(&lookup))
        .await?
        .ok_or_else(|| Error::UserNotFound(name.clone()))
}

pub(crate) async fn list(state: &AppState) -> Result<Vec<User>> {
    state.db.call(|store| store.list_users()).await
}

pub(crate) async fn create(
    state: &AppState,
    name: &Name,
    role: UserRole,
) -> Result<(User, Issued)> {
    let first: Name = FIRST_TOKEN.parse()?;
    let secret = new_secret()?;
    let (owned_name, hashed, prefix) = (name.clone(), hash(&secret), shown(&secret));
    let (user, token) = state
        .db
        .call(move |store| {
            store.transaction(|store| {
                let user = store.create_user(&owned_name, role)?;
                let token = store.create_token(&NewToken {
                    user_id: user.id,
                    name: &first,
                    hash: &hashed,
                    prefix: &prefix,
                    expires_at: None,
                })?;
                Ok((user, token))
            })
        })
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::AlreadyExists(_)) => Error::UserExists(name.clone()),
            other => other,
        })?;
    tracing::info!(user = %name, role = %role, "user created");
    Ok((user, Issued { token, secret }))
}

pub(crate) async fn remove(state: &AppState, name: &Name) -> Result<()> {
    let user = find(state, name).await?;
    state
        .db
        .call(move |store| store.delete_user(user.id))
        .await?;
    tracing::info!(user = %name, "user deleted with their tokens");
    Ok(())
}

pub(crate) async fn tokens(state: &AppState, user: &User) -> Result<Vec<ApiToken>> {
    let id = user.id;
    state.db.call(move |store| store.list_tokens(id)).await
}

pub(crate) async fn issue(
    state: &AppState,
    user: &User,
    name: &Name,
    expires_in_days: Option<NonZeroU16>,
) -> Result<Issued> {
    let secret = new_secret()?;
    let expires_at =
        expires_in_days.map(|days| unix_now() + i64::from(days.get()) * SECONDS_PER_DAY);
    let (user_id, owned_name, hashed, prefix) =
        (user.id, name.clone(), hash(&secret), shown(&secret));
    let token = state
        .db
        .call(move |store| {
            store.create_token(&NewToken {
                user_id,
                name: &owned_name,
                hash: &hashed,
                prefix: &prefix,
                expires_at,
            })
        })
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::AlreadyExists(_)) => Error::TokenExists {
                user: user.name.clone(),
                token: name.clone(),
            },
            other => other,
        })?;
    tracing::info!(user = %user.name, token = %name, "token created");
    Ok(Issued { token, secret })
}

pub(crate) async fn revoke(state: &AppState, user: &User, name: &Name) -> Result<()> {
    let (user_id, owned_name) = (user.id, name.clone());
    state
        .db
        .call(move |store| store.delete_token(user_id, &owned_name))
        .await
        .map_err(|err| match err {
            Error::Store(bird_store::Error::NotFound(_)) => Error::TokenNotFound {
                user: user.name.clone(),
                token: name.clone(),
            },
            other => other,
        })?;
    tracing::info!(user = %user.name, token = %name, "token deleted");
    Ok(())
}

fn new_secret() -> Result<String> {
    Ok(format!(
        "{TOKEN_PREFIX}{}",
        secrets::generate(SECRET_CHARS)?
    ))
}

fn shown(secret: &str) -> String {
    secret.chars().take(SHOWN_CHARS).collect()
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| {
            i64::try_from(elapsed.as_secs()).unwrap_or(i64::MAX)
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_and_shows_only_a_prefix() {
        assert_eq!(
            hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
        let secret = new_secret().unwrap();
        assert!(secret.starts_with("bird_"));
        assert_eq!(secret.len(), TOKEN_PREFIX.len() + SECRET_CHARS);
        assert_eq!(shown(&secret).len(), SHOWN_CHARS);
        assert_ne!(new_secret().unwrap(), secret);
    }
}
