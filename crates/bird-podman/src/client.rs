use std::path::{Path, PathBuf};
use std::time::Duration;

use hyper::Method;
use serde::Serialize;

use crate::Result;
use crate::error::check;
use crate::transport::{Response, Streamed, Transport};

pub(crate) const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

const SYSTEM_SOCKET: &str = "/run/podman/podman.sock";

#[derive(Debug, Clone)]
pub struct Podman {
    transport: Transport,
}

impl Podman {
    #[must_use]
    pub fn new(socket: impl Into<PathBuf>) -> Self {
        Self {
            transport: Transport::new(socket.into()),
        }
    }

    #[must_use]
    pub fn socket(&self) -> &Path {
        self.transport.socket()
    }

    pub async fn ping(&self) -> Result<()> {
        let response = self.get("/_ping").await?;
        check(response, || "podman api".to_owned())?;
        Ok(())
    }

    pub(crate) async fn get(&self, path: &str) -> Result<Response> {
        self.transport
            .send(Method::GET, path, None, DEFAULT_TIMEOUT)
            .await
    }

    pub(crate) async fn post_json(&self, path: &str, body: &impl Serialize) -> Result<Response> {
        let body = serde_json::to_vec(body)?;
        self.transport
            .send(Method::POST, path, Some(body), DEFAULT_TIMEOUT)
            .await
    }

    pub(crate) async fn upload<B>(
        &self,
        path: &str,
        content_type: &str,
        body: B,
        timeout: Duration,
    ) -> Result<Streamed>
    where
        B: hyper::body::Body<Data = bytes::Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        self.transport
            .upload(path, content_type, body, timeout)
            .await
    }

    pub(crate) async fn send_with_registry_auth(
        &self,
        method: Method,
        path: &str,
        registry_auth: String,
        timeout: Duration,
    ) -> Result<Response> {
        self.transport
            .send_with_registry_auth(method, path, registry_auth, timeout)
            .await
    }

    pub(crate) async fn stream(&self, path: &str) -> Result<Streamed> {
        self.transport.stream(path, DEFAULT_TIMEOUT).await
    }

    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        timeout: Duration,
    ) -> Result<Response> {
        self.transport.send(method, path, None, timeout).await
    }
}

#[must_use]
pub fn default_socket() -> PathBuf {
    std::env::var_os("XDG_RUNTIME_DIR")
        .map(|dir| PathBuf::from(dir).join("podman/podman.sock"))
        .filter(|path| path.exists())
        .unwrap_or_else(|| PathBuf::from(SYSTEM_SOCKET))
}
