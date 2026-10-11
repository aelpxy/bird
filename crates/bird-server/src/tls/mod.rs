mod account;
mod expiry;
mod failures;
mod issue;
mod manager;

use std::path::PathBuf;

use bird_proxy::CertStore;

use crate::Result;
use crate::state::AppState;

pub(crate) use failures::{CertFailures, Failure};
pub(crate) use manager::CertManager;

#[derive(Debug, Clone)]
pub(crate) struct AcmeSettings {
    pub(crate) directory: String,
    pub(crate) email: Option<String>,
    pub(crate) ca_cert: Option<PathBuf>,
}

pub(crate) async fn load_certificates(state: &AppState, store: &CertStore) -> Result<()> {
    let also = state.dashboard_domain.clone();
    let certificates = state
        .db
        .call(move |store| store.list_certificates(also.as_ref()))
        .await?;
    let loaded = store.replace(&certificates);
    tracing::debug!(loaded, "tls certificates loaded");
    Ok(())
}
