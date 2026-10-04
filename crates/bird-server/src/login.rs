use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bird_core::{Name, Session, User};
use bird_store::{LoginSecrets, NewSession};

use crate::state::AppState;
use crate::throttle::Key;
use crate::{Error, Result, passwords, secrets, totp, users};

// session secrets look different from api tokens, so the middleware knows where to look
pub(crate) const SESSION_PREFIX: &str = "birds_";
const SESSION_CHARS: usize = 40;
pub(crate) const SESSION_LIFETIME: Duration = Duration::from_hours(24 * 30);
pub(crate) const SESSION_IDLE: Duration = Duration::from_hours(24 * 7);

pub(crate) struct Attempt {
    pub(crate) username: Name,
    pub(crate) password: String,
    pub(crate) code: Option<String>,
    pub(crate) address: Option<String>,
    pub(crate) agent: Option<String>,
}

pub(crate) enum Outcome {
    SignedIn {
        token: String,
        session: Session,
        user: User,
    },
    // the password was right; send it again with a code
    TwoFactorRequired,
}

pub(crate) async fn sign_in(state: &AppState, attempt: Attempt) -> Result<Outcome> {
    let now = Instant::now();
    let user_key = Key::User(attempt.username.to_string());
    let keys = [
        user_key.clone(),
        Key::Address(attempt.address.clone().unwrap_or_default()),
    ];
    if let Some(wait) = state.logins.wait(&keys, now) {
        return Err(Error::TooManyAttempts(wait.as_secs().max(1)));
    }
    let lookup = attempt.username.clone();
    let found = state
        .db
        .call(move |store| {
            let Some(user) = store.user_by_name(&lookup)? else {
                return Ok(None);
            };
            let secrets = store.login_secrets(user.id)?;
            Ok(Some((user, secrets)))
        })
        .await?;
    let (user, secrets) = found.map_or((None, LoginSecrets::default()), |(u, s)| (Some(u), s));
    let matched = passwords::verify(attempt.password, secrets.password_hash).await;
    let Some(user) = user.filter(|_| matched) else {
        state.logins.failed(&keys, now);
        tracing::warn!(user = %attempt.username, address = ?attempt.address, "failed sign-in");
        return Err(Error::InvalidLogin);
    };
    if let Some(secret) = secrets.totp_secret {
        let Some(code) = attempt.code else {
            return Ok(Outcome::TwoFactorRequired);
        };
        if !second_factor(state, &user, &secret, &code).await? {
            state.logins.failed(&keys, now);
            tracing::warn!(user = %user.name, address = ?attempt.address, "wrong two-factor code");
            return Err(Error::InvalidCode);
        }
    }
    state.logins.succeeded(&user_key);
    let (token, session) = open_session(state, &user, attempt.address, attempt.agent).await?;
    tracing::info!(user = %user.name, address = ?session.address, "signed in");
    Ok(Outcome::SignedIn {
        token,
        session,
        user,
    })
}

// a code from the authenticator, accepted once per step, or an unused recovery code
async fn second_factor(state: &AppState, user: &User, secret: &str, code: &str) -> Result<bool> {
    let user_id = user.id;
    if let Some(step) = totp::verify(secret, code, unix_now()) {
        return state
            .db
            .call(move |store| store.accept_totp_step(user_id, step))
            .await;
    }
    let hashed = users::hash(&normalize_recovery(code));
    let recovered = state
        .db
        .call(move |store| store.use_recovery_code(user_id, &hashed))
        .await?;
    if recovered {
        tracing::warn!(user = %user.name, "signed in with a recovery code");
    }
    Ok(recovered)
}

async fn open_session(
    state: &AppState,
    user: &User,
    address: Option<String>,
    agent: Option<String>,
) -> Result<(String, Session)> {
    let token = format!("{SESSION_PREFIX}{}", secrets::generate(SESSION_CHARS)?);
    let hashed = users::hash(&token);
    let expires_at = unix_now() + i64::try_from(SESSION_LIFETIME.as_secs()).unwrap_or(i64::MAX);
    let user_id = user.id;
    let session = state
        .db
        .call(move |store| {
            store.create_session(&NewSession {
                user_id,
                hash: &hashed,
                expires_at,
                address: address.as_deref(),
                agent: agent.as_deref(),
            })
        })
        .await?;
    Ok((token, session))
}

// recovery codes are shown as `xxxx-xxxx`; case, dashes and spaces do not matter when typed
pub(crate) fn normalize_recovery(code: &str) -> String {
    code.chars()
        .filter(char::is_ascii_alphanumeric)
        .map(|c| c.to_ascii_lowercase())
        .collect()
}

pub(crate) fn unix_now() -> i64 {
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
    fn recovery_codes_ignore_case_and_dashes() {
        assert_eq!(normalize_recovery("AB12-cd34"), "ab12cd34");
        assert_eq!(normalize_recovery(" ab12 cd34 "), "ab12cd34");
    }
}
