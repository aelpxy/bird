use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;

use arc_swap::ArcSwap;
use bird_core::Certificate;
use rustls::ServerConfig;
use rustls::crypto::CryptoProvider;
use rustls::crypto::ring::default_provider;
use rustls::pki_types::pem::PemObject;
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use rustls::server::{ClientHello, ResolvesServerCert};
use rustls::sign::CertifiedKey;

use crate::Challenges;

#[derive(Clone)]
pub struct Tls {
    pub certificates: CertStore,
    pub challenges: Challenges,
    pub https_port: u16,
}

#[derive(Clone, Default)]
pub struct CertStore {
    keys: Arc<ArcSwap<HashMap<String, Arc<CertifiedKey>>>>,
}

impl CertStore {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace(&self, certificates: &[Certificate]) -> usize {
        let provider = default_provider();
        let mut keys = HashMap::with_capacity(certificates.len());
        for certificate in certificates {
            match certified_key(certificate, &provider) {
                Ok(key) => {
                    keys.insert(certificate.hostname.to_string(), Arc::new(key));
                }
                Err(err) => {
                    tracing::warn!(hostname = %certificate.hostname, error = %err, "skipping unusable certificate");
                }
            }
        }
        let loaded = keys.len();
        self.keys.store(Arc::new(keys));
        loaded
    }

    pub(crate) fn contains(&self, host: &str) -> bool {
        self.keys.load().contains_key(host)
    }

    fn get(&self, host: &str) -> Option<Arc<CertifiedKey>> {
        self.keys.load().get(host).cloned()
    }
}

impl fmt::Debug for CertStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CertStore({} hosts)", self.keys.load().len())
    }
}

fn certified_key(
    certificate: &Certificate,
    provider: &CryptoProvider,
) -> Result<CertifiedKey, rustls::Error> {
    let invalid = |err: rustls::pki_types::pem::Error| rustls::Error::General(err.to_string());
    let chain = CertificateDer::pem_slice_iter(certificate.chain_pem.as_bytes())
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid)?;
    if chain.is_empty() {
        return Err(rustls::Error::General(
            "certificate chain is empty".to_owned(),
        ));
    }
    let key = PrivateKeyDer::from_pem_slice(certificate.key_pem.as_bytes()).map_err(invalid)?;
    CertifiedKey::from_der(chain, key, provider)
}

#[derive(Debug)]
struct Resolver(CertStore);

impl ResolvesServerCert for Resolver {
    fn resolve(&self, client_hello: ClientHello<'_>) -> Option<Arc<CertifiedKey>> {
        let name = client_hello.server_name()?;
        self.0.get(&name.to_ascii_lowercase())
    }
}

pub(crate) fn server_config(store: CertStore) -> Result<Arc<ServerConfig>, rustls::Error> {
    let mut config = ServerConfig::builder_with_provider(Arc::new(default_provider()))
        .with_safe_default_protocol_versions()?
        .with_no_client_auth()
        .with_cert_resolver(Arc::new(Resolver(store)));
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(config))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skips_invalid_certificates() {
        let store = CertStore::new();
        let broken = Certificate {
            hostname: "web.example.com".parse().unwrap(),
            chain_pem: "not a pem".to_owned(),
            key_pem: "not a key".to_owned(),
            not_after: 0,
        };
        assert_eq!(store.replace(&[broken]), 0);
        assert!(!store.contains("web.example.com"));
    }
}
