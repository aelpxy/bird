use std::net::SocketAddr;
use std::sync::Arc;

use bird_core::Certificate;
use bird_proxy::{CertStore, Challenges, Proxy, ProxyConfig, Routes, Tls};
use bytes::Bytes;
use http_body_util::Full;
use hyper::Request;
use rcgen::{BasicConstraints, CertificateParams, CertifiedIssuer, IsCa, KeyPair};
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, RootCertStore};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;
use tokio_rustls::client::TlsStream;

use super::{Reply, send_on};

pub const SECURE: &str = "secure.localhost";
pub const PLAIN: &str = "plain.localhost";

pub struct Edge {
    pub http: SocketAddr,
    pub https: SocketAddr,
    pub ca: CertificateDer<'static>,
    pub challenges: Challenges,
}

fn issue(host: &str) -> (Certificate, CertificateDer<'static>) {
    let mut ca_params = CertificateParams::new(Vec::<String>::new()).unwrap();
    ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca = CertifiedIssuer::self_signed(ca_params, KeyPair::generate().unwrap()).unwrap();
    let key = KeyPair::generate().unwrap();
    let leaf = CertificateParams::new(vec![host.to_owned()])
        .unwrap()
        .signed_by(&key, &ca)
        .unwrap();
    let certificate = Certificate {
        hostname: host.parse().unwrap(),
        chain_pem: leaf.pem(),
        key_pem: key.serialize_pem(),
        not_after: 0,
    };
    (certificate, ca.der().clone())
}

pub fn spawn_edge(routes: Routes) -> Edge {
    let plain_listener = bird_proxy::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let secure_listener = bird_proxy::bind("127.0.0.1:0".parse().unwrap()).unwrap();
    let http = plain_listener.local_addr().unwrap();
    let https = secure_listener.local_addr().unwrap();

    let (certificate, ca) = issue(SECURE);
    let certificates = CertStore::new();
    assert_eq!(certificates.replace(&[certificate]), 1);
    let challenges = Challenges::new();
    let tls = Tls {
        certificates,
        challenges: challenges.clone(),
        https_port: https.port(),
    };

    let proxy = Arc::new(Proxy::new(routes, ProxyConfig::default(), Some(tls)));
    let plain = Arc::clone(&proxy);
    tokio::spawn(async move { plain.serve(plain_listener, std::future::pending()).await });
    tokio::spawn(async move {
        proxy
            .serve_tls(secure_listener, std::future::pending())
            .await
            .unwrap();
    });
    Edge {
        http,
        https,
        ca,
        challenges,
    }
}

pub async fn tls_stream(edge: &Edge, sni: &str) -> std::io::Result<TlsStream<TcpStream>> {
    let mut roots = RootCertStore::empty();
    roots.add(edge.ca.clone()).unwrap();
    let config =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
    let tcp = TcpStream::connect(edge.https).await?;
    let name = ServerName::try_from(sni.to_owned()).unwrap();
    TlsConnector::from(Arc::new(config))
        .connect(name, tcp)
        .await
}

pub async fn https_get(edge: &Edge, sni: &str, path: &str) -> std::io::Result<Reply> {
    let stream = tls_stream(edge, sni).await?;
    let request = Request::get(path)
        .header("host", sni)
        .body(Full::new(Bytes::new()))
        .unwrap();
    Ok(send_on(stream, request).await)
}
