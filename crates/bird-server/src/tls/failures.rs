use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use bird_core::Hostname;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Failure {
    pub(crate) error: String,
    pub(crate) retry_at: i64,
}

// the latest failed certificate request per hostname, kept in memory so the api can show it
#[derive(Debug, Default)]
pub(crate) struct CertFailures(Mutex<HashMap<Hostname, Failure>>);

impl CertFailures {
    pub(crate) fn record(&self, hostname: Hostname, failure: Failure) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(hostname, failure);
    }

    pub(crate) fn clear(&self, hostname: &Hostname) {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(hostname);
    }

    pub(crate) fn get(&self, hostname: &Hostname) -> Option<Failure> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(hostname)
            .cloned()
    }
}
