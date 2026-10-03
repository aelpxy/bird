use std::fmt;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use bird_api::ErrorBody;
use bytes::Bytes;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::client::conn::http1::{Connection, SendRequest};
use hyper::header::{AUTHORIZATION, CONTENT_TYPE, HOST};
use hyper::{Method, Request, StatusCode};
use hyper_util::rt::TokioIo;
use serde::Serialize;
use serde::de::DeserializeOwned;
use tokio::net::TcpStream;

const JSON: &str = "application/json";

pub(crate) struct ApiClient {
    addr: String,
    token: Option<String>,
}

#[derive(Debug)]
struct ApiError {
    status: StatusCode,
    body: ErrorBody,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.body.error, self.status)?;
        if self.status == StatusCode::UNAUTHORIZED {
            write!(
                f,
                "\nrun `bird login <host:port>` with the token from birdd's data dir"
            )?;
        }
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
    pub(crate) fn new(addr: String, token: Option<String>) -> Self {
        Self { addr, token }
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

    pub(crate) async fn patch<B: Serialize, T: DeserializeOwned>(
        &self,
        path: &str,
        payload: &B,
        timeout: Duration,
    ) -> Result<T> {
        let payload = serde_json::to_vec(payload)?;
        let body = self
            .send(Method::PATCH, path, Some(payload), timeout)
            .await?;
        serde_json::from_slice(&body).context("birdd sent an unexpected response")
    }

    pub(crate) async fn put<B: Serialize>(
        &self,
        path: &str,
        payload: &B,
        timeout: Duration,
    ) -> Result<()> {
        let payload = serde_json::to_vec(payload)?;
        self.send(Method::PUT, path, Some(payload), timeout).await?;
        Ok(())
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

    // the timeout covers the response head; the body then streams until birdd or the user stops it
    pub(crate) async fn stream_lines(
        &self,
        path: &str,
        timeout: Duration,
        on_line: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let request = self.request(Method::GET, path, None, JSON)?;
        self.read_lines(request, timeout, on_line).await
    }

    // sends a raw body and streams the answer line by line, like stream_lines
    pub(crate) async fn upload_lines(
        &self,
        path: &str,
        content_type: &str,
        payload: Vec<u8>,
        timeout: Duration,
        on_line: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let request = self.request(Method::POST, path, Some(payload), content_type)?;
        self.read_lines(request, timeout, on_line).await
    }

    async fn read_lines(
        &self,
        request: Request<Full<Bytes>>,
        timeout: Duration,
        mut on_line: impl FnMut(&str) -> Result<()>,
    ) -> Result<()> {
        let response = tokio::time::timeout(timeout, self.open_stream(request))
            .await
            .map_err(|_| anyhow!("birdd did not respond within {}s", timeout.as_secs()))??;
        let status = response.status();
        let mut body = response.into_body();
        if !status.is_success() {
            let body = body.collect().await?.to_bytes();
            return Err(api_error(status, &body));
        }
        let mut buffer = Vec::new();
        while let Some(frame) = body.frame().await {
            let Ok(data) = frame?.into_data() else {
                continue;
            };
            buffer.extend_from_slice(&data);
            while let Some(end) = buffer.iter().position(|byte| *byte == b'\n') {
                let mut line: Vec<u8> = buffer.drain(..=end).collect();
                line.pop();
                on_line(String::from_utf8_lossy(&line).as_ref())?;
            }
        }
        Ok(())
    }

    async fn open_stream(
        &self,
        request: Request<Full<Bytes>>,
    ) -> Result<hyper::Response<Incoming>> {
        let (mut sender, conn) = self.connect().await?;
        tokio::spawn(conn);
        Ok(sender.send_request(request).await?)
    }

    async fn connect(
        &self,
    ) -> Result<(
        SendRequest<Full<Bytes>>,
        Connection<TokioIo<TcpStream>, Full<Bytes>>,
    )> {
        let stream = TcpStream::connect(&self.addr)
            .await
            .with_context(|| format!("cannot reach birdd at {}, is it running?", self.addr))?;
        Ok(hyper::client::conn::http1::handshake(TokioIo::new(stream)).await?)
    }

    fn request(
        &self,
        method: Method,
        path: &str,
        payload: Option<Vec<u8>>,
        content_type: &str,
    ) -> Result<Request<Full<Bytes>>> {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(HOST, &self.addr);
        if let Some(token) = &self.token {
            request = request.header(AUTHORIZATION, format!("Bearer {token}"));
        }
        if payload.is_some() {
            request = request.header(CONTENT_TYPE, content_type);
        }
        Ok(request.body(Full::new(Bytes::from(payload.unwrap_or_default())))?)
    }

    async fn exchange(
        &self,
        method: Method,
        path: &str,
        payload: Option<Vec<u8>>,
    ) -> Result<Bytes> {
        let (mut sender, conn) = self.connect().await?;
        let request = self.request(method, path, payload, JSON)?;

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
        Err(api_error(status, &body))
    }
}

fn api_error(status: StatusCode, body: &[u8]) -> anyhow::Error {
    let body = serde_json::from_slice::<ErrorBody>(body).unwrap_or_else(|_| ErrorBody {
        error: String::from_utf8_lossy(body).trim().to_owned(),
        logs: Vec::new(),
    });
    ApiError { status, body }.into()
}
