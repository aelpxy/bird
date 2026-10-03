use std::collections::HashMap;
use std::future::Future;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use bird_core::Hostname;
use bird_proxy::{CertStore, Challenges};
use instant_acme::Account;

use super::{AcmeSettings, account, issue, load_certificates};
use crate::Result;
use crate::backoff::Backoff;
use crate::state::AppState;

const CHECK_INTERVAL: Duration = Duration::from_hours(1);
// public CAs rate limit failed validations, so retries start slow and stay slow
const RETRY_BASE: Duration = Duration::from_mins(5);
const RETRY_MAX: Duration = Duration::from_hours(12);
const RENEW_BEFORE_SECS: i64 = 30 * 24 * 60 * 60;
const UNCERTIFIABLE_SUFFIX: &str = ".localhost";

pub(crate) struct CertManager {
    state: AppState,
    settings: AcmeSettings,
    certificates: CertStore,
    challenges: Challenges,
    account: Option<Account>,
    account_backoff: Backoff,
    backoff: HashMap<Hostname, Backoff>,
}

impl CertManager {
    pub(crate) fn new(
        state: AppState,
        settings: AcmeSettings,
        certificates: CertStore,
        challenges: Challenges,
    ) -> Self {
        Self {
            state,
            settings,
            certificates,
            challenges,
            account: None,
            account_backoff: Backoff::new(RETRY_BASE, RETRY_MAX),
            backoff: HashMap::new(),
        }
    }

    pub(crate) async fn run(mut self, shutdown: impl Future<Output = ()>) {
        let domains_changed = std::sync::Arc::clone(&self.state.domains_changed);
        tokio::pin!(shutdown);
        loop {
            tokio::select! {
                () = &mut shutdown => break,
                () = self.sweep() => {}
            }
            tokio::select! {
                () = &mut shutdown => break,
                () = tokio::time::sleep_until(self.next_wake().into()) => {}
                () = domains_changed.notified() => {}
            }
        }
        tracing::info!("certificate manager stopped");
    }

    async fn sweep(&mut self) {
        let due = match self.due_hostnames().await {
            Ok(due) => due,
            Err(err) => {
                tracing::warn!(error = %err, "could not check certificates");
                return;
            }
        };
        for hostname in due {
            let ready = self
                .backoff
                .get(&hostname)
                .is_none_or(|backoff| backoff.ready(Instant::now()));
            if !ready {
                continue;
            }
            let mut outcome = self.obtain(&hostname).await;
            if outcome
                .as_ref()
                .is_some_and(|result| result.as_ref().is_err_and(account::is_gone))
            {
                tracing::warn!("acme account no longer exists at the ca, registering a new one");
                self.forget_account().await;
                outcome = self.obtain(&hostname).await;
            }
            let Some(result) = outcome else {
                break;
            };
            let backoff = self
                .backoff
                .entry(hostname.clone())
                .or_insert_with(|| Backoff::new(RETRY_BASE, RETRY_MAX));
            match result {
                Ok(not_after) => {
                    backoff.reset();
                    tracing::info!(%hostname, not_after, "certificate issued");
                }
                Err(err) => {
                    let wait = backoff.fail(Instant::now());
                    tracing::warn!(%hostname, error = %err, retry_in_secs = wait.as_secs(), "certificate request failed");
                }
            }
        }
        // reloading every sweep also drops certificates of domains that were removed
        if let Err(err) = load_certificates(&self.state, &self.certificates).await {
            tracing::warn!(error = %err, "could not reload certificates");
        }
    }

    async fn obtain(&mut self, hostname: &Hostname) -> Option<Result<i64>> {
        let account = self.account().await?;
        tracing::info!(%hostname, "requesting certificate");
        let result = match issue::issue(&account, &self.challenges, hostname).await {
            Ok(certificate) => {
                let not_after = certificate.not_after;
                self.state
                    .db
                    .call(move |store| store.put_certificate(&certificate))
                    .await
                    .map(|()| not_after)
            }
            Err(err) => Err(err),
        };
        Some(result)
    }

    async fn forget_account(&mut self) {
        self.account = None;
        let directory = self.settings.directory.clone();
        let forgotten = self
            .state
            .db
            .call(move |store| store.forget_acme_credentials(&directory))
            .await;
        if let Err(err) = forgotten {
            tracing::warn!(error = %err, "could not forget stale acme account");
        }
    }

    async fn due_hostnames(&self) -> Result<Vec<Hostname>> {
        let (hostnames, certificates) = self
            .state
            .db
            .call(|store| Ok((store.list_hostnames()?, store.list_certificates()?)))
            .await?;
        let expiry: HashMap<Hostname, i64> = certificates
            .into_iter()
            .map(|c| (c.hostname, c.not_after))
            .collect();
        Ok(due(hostnames, &expiry, unix_now()))
    }

    fn next_wake(&self) -> Instant {
        let retries = self
            .backoff
            .values()
            .chain(std::iter::once(&self.account_backoff))
            .filter_map(Backoff::next_attempt);
        earliest_wake(Instant::now(), retries)
    }

    async fn account(&mut self) -> Option<Account> {
        if self.account.is_none() && self.account_backoff.ready(Instant::now()) {
            match account::load_or_create(&self.state, &self.settings).await {
                Ok(account) => {
                    self.account_backoff.reset();
                    self.account = Some(account);
                }
                Err(err) => {
                    let wait = self.account_backoff.fail(Instant::now());
                    tracing::warn!(error = %err, retry_in_secs = wait.as_secs(), "acme account unavailable");
                }
            }
        }
        self.account.clone()
    }
}

fn due(hostnames: Vec<Hostname>, expiry: &HashMap<Hostname, i64>, now: i64) -> Vec<Hostname> {
    hostnames
        .into_iter()
        .filter(|h| !h.as_str().ends_with(UNCERTIFIABLE_SUFFIX))
        .filter(|h| {
            expiry
                .get(h)
                .is_none_or(|&not_after| not_after - now < RENEW_BEFORE_SECS)
        })
        .collect()
}

fn earliest_wake(now: Instant, retries: impl Iterator<Item = Instant>) -> Instant {
    retries
        .filter(|at| *at > now)
        .fold(now + CHECK_INTERVAL, Instant::min)
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_secs()).unwrap_or(i64::MAX))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn host(raw: &str) -> Hostname {
        raw.parse().unwrap()
    }

    #[test]
    fn wakes_for_the_earliest_pending_retry() {
        let now = Instant::now();
        let soon = now + Duration::from_mins(5);
        let later = now + Duration::from_mins(20);
        let past = now.checked_sub(Duration::from_secs(1)).unwrap();
        assert_eq!(earliest_wake(now, [later, soon, past].into_iter()), soon);
        assert_eq!(earliest_wake(now, std::iter::empty()), now + CHECK_INTERVAL);
    }

    #[test]
    fn picks_missing_and_expiring_public_hosts() {
        let now = 1_000_000_000;
        let expiry = HashMap::from([
            (host("fresh.example.com"), now + RENEW_BEFORE_SECS + 1),
            (host("expiring.example.com"), now + RENEW_BEFORE_SECS - 1),
        ]);
        let hosts = vec![
            host("fresh.example.com"),
            host("expiring.example.com"),
            host("new.example.com"),
            host("web.localhost"),
        ];
        assert_eq!(
            due(hosts, &expiry, now),
            vec![host("expiring.example.com"), host("new.example.com")]
        );
    }
}
