use bird_core::{SessionId, User};

use crate::login::{normalize_recovery, unix_now};
use crate::state::AppState;
use crate::{Error, Result, passwords, secrets, totp, users};

const RECOVERY_CODES: usize = 10;
const RECOVERY_BYTES: usize = 5;

// `current` is required when the user changes their own password and already has one; every
// other session of theirs is signed out, except `keep`
pub(crate) async fn set_password(
    state: &AppState,
    user: &User,
    current: Option<String>,
    new: String,
    keep: Option<SessionId>,
) -> Result<()> {
    passwords::check_policy(&new)?;
    if let Some(current) = current {
        check_password(state, user, current).await?;
    }
    let hashed = passwords::hash(new).await?;
    let user_id = user.id;
    let signed_out = state
        .db
        .call(move |store| {
            store.set_password_hash(user_id, &hashed)?;
            store.delete_sessions(user_id, keep)
        })
        .await?;
    tracing::info!(user = %user.name, signed_out, "password changed");
    Ok(())
}

pub(crate) async fn check_password(state: &AppState, user: &User, password: String) -> Result<()> {
    let user_id = user.id;
    let secrets = state
        .db
        .call(move |store| store.login_secrets(user_id))
        .await?;
    if passwords::verify(password, secrets.password_hash).await {
        Ok(())
    } else {
        Err(Error::WrongPassword)
    }
}

// a new secret to scan; sign-ins keep using the old one, if any, until it is confirmed
pub(crate) async fn start_two_factor(state: &AppState, user: &User) -> Result<(String, String)> {
    let secret = totp::new_secret()?;
    let (user_id, pending) = (user.id, secret.clone());
    state
        .db
        .call(move |store| store.set_totp_pending(user_id, &pending))
        .await?;
    let uri = totp::uri(&secret, user.name.as_str());
    Ok((secret, uri))
}

// turns 2fa on once a code from the new secret checks out, and returns the recovery codes,
// shown only this once
pub(crate) async fn confirm_two_factor(
    state: &AppState,
    user: &User,
    code: &str,
) -> Result<Vec<String>> {
    let user_id = user.id;
    let secrets = state
        .db
        .call(move |store| store.login_secrets(user_id))
        .await?;
    let pending = secrets.totp_pending.ok_or(Error::NoPendingTwoFactor)?;
    let step = totp::verify(&pending, code, unix_now()).ok_or(Error::InvalidCode)?;
    let codes = recovery_codes()?;
    let hashes: Vec<String> = codes
        .iter()
        .map(|code| users::hash(&normalize_recovery(code)))
        .collect();
    state
        .db
        .call(move |store| {
            store.enable_totp(user_id, &hashes)?;
            // the code just typed must not also sign someone in
            store.accept_totp_step(user_id, step)
        })
        .await?;
    tracing::info!(user = %user.name, "two-factor turned on");
    Ok(codes)
}

pub(crate) async fn disable_two_factor(state: &AppState, user: &User) -> Result<()> {
    let user_id = user.id;
    state
        .db
        .call(move |store| store.disable_totp(user_id))
        .await?;
    tracing::info!(user = %user.name, "two-factor turned off");
    Ok(())
}

fn recovery_codes() -> Result<Vec<String>> {
    (0..RECOVERY_CODES)
        .map(|_| {
            let raw = secrets::random_bytes(RECOVERY_BYTES)?;
            let code = totp::encode(&raw).to_ascii_lowercase();
            let (left, right) = code.split_at(code.len() / 2);
            Ok(format!("{left}-{right}"))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recovery_codes_are_distinct_and_typeable() {
        let codes = recovery_codes().unwrap();
        assert_eq!(codes.len(), RECOVERY_CODES);
        let unique: std::collections::HashSet<_> = codes.iter().collect();
        assert_eq!(unique.len(), RECOVERY_CODES);
        assert!(
            codes
                .iter()
                .all(|code| code.len() == 9 && code.chars().nth(4) == Some('-'))
        );
    }
}
