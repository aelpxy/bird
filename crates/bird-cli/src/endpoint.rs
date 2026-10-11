use std::sync::{Arc, OnceLock};

use anyhow::{Context, Result, anyhow};
use rustls::ClientConfig;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

pub(crate) trait Io: AsyncRead + AsyncWrite + Unpin + Send {}
impl<T: AsyncRead + AsyncWrite + Unpin + Send> Io for T {}

/// Where birdd answers: a bare `host:port` is its own plain http api, an `https://` url is
/// birdd behind its proxy at `--dashboard-domain`
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Endpoint {
    pub(crate) authority: String,
    connect: String,
    tls: Option<String>,
}

impl Endpoint {
    pub(crate) fn parse(api: &str) -> Result<Self> {
        let (rest, tls) = if let Some(rest) = api.strip_prefix("https://") {
            (rest, true)
        } else {
            (api.strip_prefix("http://").unwrap_or(api), false)
        };
        let authority = rest.trim_end_matches('/');
        if authority.is_empty() || authority.contains('/') {
            return Err(anyhow!(
                "invalid birdd address {api:?}: use host:port, or https://host for a dashboard domain"
            ));
        }
        let host = if authority.ends_with(']') {
            authority
        } else {
            authority
                .rsplit_once(':')
                .map_or(authority, |(host, _)| host)
        };
        let connect = if authority.len() == host.len() {
            format!("{authority}:{}", if tls { 443 } else { 80 })
        } else {
            authority.to_owned()
        };
        Ok(Self {
            authority: authority.to_owned(),
            connect,
            tls: tls.then(|| {
                host.trim_start_matches('[')
                    .trim_end_matches(']')
                    .to_owned()
            }),
        })
    }

    pub(crate) async fn connect(&self) -> Result<Box<dyn Io>> {
        let stream = TcpStream::connect(&self.connect).await.with_context(|| {
            format!(
                "cannot reach birdd at {}; is it running (systemctl --user status birdd), or is --api set to the right address?",
                self.authority
            )
        })?;
        let Some(host) = &self.tls else {
            return Ok(Box::new(stream));
        };
        let name = ServerName::try_from(host.clone())
            .with_context(|| format!("{host} is not a valid server name"))?;
        let tls = TlsConnector::from(client_config()?)
            .connect(name, stream)
            .await
            .with_context(|| format!("tls with {} failed", self.authority))?;
        Ok(Box::new(tls))
    }
}

// loaded once: the system's roots, speaking http/1.1 only since that is all the client does
fn client_config() -> Result<Arc<ClientConfig>> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    if let Some(config) = CONFIG.get() {
        return Ok(Arc::clone(config));
    }
    let mut roots = rustls::RootCertStore::empty();
    let found = rustls_native_certs::load_native_certs();
    for cert in found.certs {
        // one unreadable certificate in the system store should not stop the rest
        let _ = roots.add(cert);
    }
    if roots.is_empty() {
        return Err(anyhow!(
            "no trusted certificates found on this system; install ca-certificates"
        ));
    }
    let mut config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()?
            .with_root_certificates(roots)
            .with_no_client_auth();
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::clone(CONFIG.get_or_init(|| Arc::new(config))))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_plain_addresses_and_https_urls() {
        let plain = Endpoint::parse("127.0.0.1:7070").unwrap();
        assert_eq!(
            (plain.authority.as_str(), plain.connect.as_str(), plain.tls),
            ("127.0.0.1:7070", "127.0.0.1:7070", None)
        );
        let https = Endpoint::parse("https://bird.example.com/").unwrap();
        assert_eq!(https.authority, "bird.example.com");
        assert_eq!(https.connect, "bird.example.com:443");
        assert_eq!(https.tls.as_deref(), Some("bird.example.com"));
        let port = Endpoint::parse("https://bird.example.com:8443").unwrap();
        assert_eq!(
            (port.connect.as_str(), port.tls.as_deref()),
            ("bird.example.com:8443", Some("bird.example.com"))
        );
        assert_eq!(
            Endpoint::parse("http://10.0.0.2:7070").unwrap().connect,
            "10.0.0.2:7070"
        );
        assert!(Endpoint::parse("https://bird.example.com/v1").is_err());
        assert!(Endpoint::parse("").is_err());
    }
}
