use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use http_body_util::{BodyExt, Full, Limited};
use hyper::body::Incoming;
use hyper::header::{CONTENT_TYPE, HOST};
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::net::UnixStream;

use crate::{Error, Result};

const API_PREFIX: &str = "/v5.0.0/libpod";
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Clone)]
pub(crate) struct Transport {
    socket: Arc<Path>,
}

pub(crate) struct Response {
    pub(crate) status: StatusCode,
    pub(crate) body: Bytes,
}

pub(crate) struct Streamed {
    pub(crate) status: StatusCode,
    pub(crate) body: Incoming,
}

impl Streamed {
    pub(crate) async fn collect(self) -> Result<Response> {
        let body = Limited::new(self.body, MAX_RESPONSE_BYTES)
            .collect()
            .await
            .map_err(Error::Body)?
            .to_bytes();
        Ok(Response {
            status: self.status,
            body,
        })
    }
}

impl Transport {
    pub(crate) fn new(socket: PathBuf) -> Self {
        Self {
            socket: socket.into(),
        }
    }

    pub(crate) fn socket(&self) -> &Path {
        &self.socket
    }

    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
        timeout: Duration,
    ) -> Result<Response> {
        tracing::debug!(%method, path, "podman request");
        tokio::time::timeout(timeout, self.exchange(method, path, body))
            .await
            .map_err(|_| Error::Timeout)?
    }

    // the timeout covers connecting and the response head; the body streams until either side stops
    pub(crate) async fn stream(&self, path: &str, timeout: Duration) -> Result<Streamed> {
        tracing::debug!(path, "podman stream");
        tokio::time::timeout(timeout, self.open_stream(path))
            .await
            .map_err(|_| Error::Timeout)?
    }

    async fn open_stream(&self, path: &str) -> Result<Streamed> {
        let stream = UnixStream::connect(&*self.socket)
            .await
            .map_err(|source| Error::Connect {
                path: self.socket.to_path_buf(),
                source,
            })?;
        let (mut sender, conn) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;
        tokio::spawn(async move {
            if let Err(err) = conn.await {
                tracing::debug!(error = %err, "podman stream connection closed");
            }
        });
        let request = Request::builder()
            .method(Method::GET)
            .uri(format!("{API_PREFIX}{path}"))
            .header(HOST, "podman")
            .body(Full::new(Bytes::new()))?;
        let response = sender.send_request(request).await?;
        Ok(Streamed {
            status: response.status(),
            body: response.into_body(),
        })
    }

    async fn exchange(
        &self,
        method: Method,
        path: &str,
        body: Option<Vec<u8>>,
    ) -> Result<Response> {
        let stream = UnixStream::connect(&*self.socket)
            .await
            .map_err(|source| Error::Connect {
                path: self.socket.to_path_buf(),
                source,
            })?;
        let (mut sender, conn) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;

        let mut builder = Request::builder()
            .method(method)
            .uri(format!("{API_PREFIX}{path}"))
            .header(HOST, "podman");
        if body.is_some() {
            builder = builder.header(CONTENT_TYPE, "application/json");
        }
        let request = builder.body(Full::new(Bytes::from(body.unwrap_or_default())))?;

        let response = async move {
            let response = sender.send_request(request).await?;
            let status = response.status();
            let body = Limited::new(response.into_body(), MAX_RESPONSE_BYTES)
                .collect()
                .await
                .map_err(Error::Body)?
                .to_bytes();
            Ok::<_, Error>(Response { status, body })
        };

        tokio::pin!(conn, response);
        tokio::select! {
            biased;
            result = &mut response => result,
            closed = &mut conn => {
                closed?;
                response.await
            }
        }
    }
}
