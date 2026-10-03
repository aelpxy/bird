use instant_acme::{Account, AccountCredentials, NewAccount};

use super::AcmeSettings;
use crate::state::AppState;
use crate::{Error, Result};

const ACCOUNT_GONE: &str = "urn:ietf:params:acme:error:accountDoesNotExist";

pub(super) async fn load_or_create(state: &AppState, settings: &AcmeSettings) -> Result<Account> {
    let directory = settings.directory.clone();
    let saved = state
        .db
        .call(move |store| store.acme_credentials(&directory))
        .await?;
    let builder = match &settings.ca_cert {
        Some(path) => Account::builder_with_root(path)?,
        None => Account::builder()?,
    };

    if let Some(saved) = saved {
        let credentials: AccountCredentials = serde_json::from_str(&saved)?;
        let account = builder.from_credentials(credentials).await?;
        tracing::info!(directory = %settings.directory, "acme account loaded");
        return Ok(account);
    }

    let contact: Vec<String> = settings
        .email
        .iter()
        .map(|email| format!("mailto:{email}"))
        .collect();
    let contact: Vec<&str> = contact.iter().map(String::as_str).collect();
    let request = NewAccount {
        contact: &contact,
        terms_of_service_agreed: true,
        only_return_existing: false,
    };
    let (account, credentials) = builder
        .create(&request, settings.directory.clone(), None)
        .await?;

    let serialized = serde_json::to_string(&credentials)?;
    let directory = settings.directory.clone();
    state
        .db
        .call(move |store| store.save_acme_credentials(&directory, &serialized))
        .await?;
    tracing::info!(directory = %settings.directory, "acme account registered");
    Ok(account)
}

pub(super) fn is_gone(err: &Error) -> bool {
    matches!(err, Error::Acme(instant_acme::Error::Api(problem)) if problem.r#type.as_deref() == Some(ACCOUNT_GONE))
}

#[cfg(test)]
mod tests {
    use instant_acme::Problem;

    use super::*;

    fn problem(kind: &str) -> Error {
        let raw = format!(r#"{{"type":"{kind}","detail":"x","status":400}}"#);
        let problem: Problem = serde_json::from_str(&raw).unwrap();
        Error::Acme(instant_acme::Error::Api(problem))
    }

    #[test]
    fn detects_missing_account() {
        assert!(is_gone(&problem(ACCOUNT_GONE)));
        assert!(!is_gone(&problem("urn:ietf:params:acme:error:rateLimited")));
        assert!(!is_gone(&Error::DbClosed));
    }
}
