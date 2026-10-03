use std::fmt;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use bird_api::ErrorBody;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::header::{CONTENT_TYPE, HOST};
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::net::TcpStream;

pub(crate) struct ApiClient {
    addr: String,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.body.error, self.status)?;
        if !self.body.logs.is_empty() {
            write!(f, "\n--- last logs ---")?;
            for line in &self.body.logs {
                write!(f, "\n{line}")?;
            }
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

impl ApiClient {
    pub(crate) fn new(addr: String) -> Self {
        Self { addr }
    }

    pub(crate) async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
        timeout: Duration,
    ) -> Result<T> {
        let body = self.send(Method::GET, path, None, timeout).await?;
        serde_json::from_slice(&body).context("birdd sent an unexpected response")
    }

    pub(crate) async fn post<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        payload: &B,
        timeout: Duration,
    ) -> Result<T> {
        let payload = serde_json::to_vec(payload)?;
        let body = self
            .send(Method::POST, path, Some(payload), timeout)
            .await?;
        serde_json::from_slice(&body).context("birdd sent an unexpected response")
    }

    pub(crate) async fn delete(&self, path: &str, timeout: Duration) -> Result<()> {
        self.send(Method::DELETE, path, None, timeout).await?;
        Ok(())
    }

    async fn send(
        &self,
        method: Method,
        path: &str,
        payload: Option<Vec<u8>>,
        timeout: Duration,
    ) -> Result<Bytes> {
        tokio::time::timeout(timeout, self.exchange(method, path, payload))
            .await
            .map_err(|_| anyhow!("birdd did not respond within {}s", timeout.as_secs()))?
    }

    async fn exchange(
        &self,
        method: Method,
        path: &str,
        payload: Option<Vec<u8>>,
    ) -> Result<Bytes> {
        let stream = TcpStream::connect(&self.addr)
            .await
            .with_context(|| format!("cannot reach birdd at {}, is it running?", self.addr))?;
        let (mut sender, conn) =
            hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?;

        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(HOST, &self.addr);
        if payload.is_some() {
            request = request.header(CONTENT_TYPE, "application/json");
        }
        let request = request.body(Full::new(Bytes::from(payload.unwrap_or_default())))?;

        let exchange = async move {
            let response = sender.send_request(request).await?;
            let status = response.status();
            let body = response.into_body().collect().await?.to_bytes();
            Ok::<_, anyhow::Error>((status, body))
        };
        tokio::pin!(conn, exchange);
        let (status, body) = tokio::select! {
            biased;
            result = &mut exchange => result?,
            closed = &mut conn => {
                closed?;
                exchange.await?
            }
        };

        if status.is_success() {
            return Ok(body);
        }
        let body = serde_json::from_slice::<ErrorBody>(&body).unwrap_or_else(|_| ErrorBody {
            error: String::from_utf8_lossy(&body).trim().to_owned(),
            logs: Vec::new(),
        });
        Err(ApiError { status, body }.into())
    }
}
