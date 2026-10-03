// shared fixtures: each test file uses a subset, and a broken fixture should fail the test loudly
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used)]

pub mod tls;

use std::convert::Infallible;
use std::net::SocketAddr;
use std::time::Duration;

use bird_core::Hostname;
use bird_proxy::{Proxy, ProxyConfig, RouteTable, Routes};
use bytes::Bytes;
use http_body_util::combinators::BoxBody;
use http_body_util::{BodyExt, Full};
use hyper::body::Incoming;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{HeaderMap, Request, Response, StatusCode};
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub const SLOW_DELAY: Duration = Duration::from_millis(500);

type Body = BoxBody<Bytes, hyper::Error>;

pub async fn spawn_upstream(name: &'static str) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = listener.accept().await else {
                continue;
            };
            tokio::spawn(async move {
                let service = service_fn(move |request| upstream_handler(name, request));
                let _ = http1::Builder::new()
                    .serve_connection(TokioIo::new(stream), service)
                    .with_upgrades()
                    .await;
            });
        }
    });
    addr
}

async fn upstream_handler(
    name: &'static str,
    request: Request<Incoming>,
) -> Result<Response<Body>, Infallible> {
    match request.uri().path() {
        "/echo" => Ok(Response::new(request.into_body().boxed())),
        "/ws" => Ok(echo_tunnel(request)),
        "/slow" => {
            tokio::time::sleep(SLOW_DELAY).await;
            Ok(text(format!("upstream={name}\nslow=done\n")))
        }
        _ => {
            let header = |key: &str| {
                request
                    .headers()
                    .get(key)
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("-")
                    .to_owned()
            };
            Ok(text(format!(
                "upstream={name}\nmethod={}\nuri={}\nhost={}\nxff={}\nxfh={}\nproto={}\nsecret={}\n",
                request.method(),
                request.uri(),
                header("host"),
                header("x-forwarded-for"),
                header("x-forwarded-host"),
                header("x-forwarded-proto"),
                header("x-secret"),
            )))
        }
    }
}

fn echo_tunnel(request: Request<Incoming>) -> Response<Body> {
    tokio::spawn(async move {
        if let Ok(upgraded) = hyper::upgrade::on(request).await {
            let (mut reader, mut writer) = tokio::io::split(TokioIo::new(upgraded));
            let _ = tokio::io::copy(&mut reader, &mut writer).await;
        }
    });
    Response::builder()
        .status(StatusCode::SWITCHING_PROTOCOLS)
        .header("connection", "upgrade")
        .header("upgrade", "websocket")
        .body(Full::new(Bytes::new()).map_err(|n| match n {}).boxed())
        .unwrap()
}

fn text(body: String) -> Response<Body> {
    Response::new(Full::new(Bytes::from(body)).map_err(|n| match n {}).boxed())
}

pub fn routes(entries: &[(&str, &[SocketAddr])]) -> Routes {
    let routes = Routes::new();
    routes.replace(table(entries));
    routes
}

pub fn table(entries: &[(&str, &[SocketAddr])]) -> RouteTable {
    let mut table = RouteTable::new();
    for (host, upstreams) in entries {
        table.insert(&host.parse::<Hostname>().unwrap(), upstreams);
    }
    table
}

pub struct RunningProxy {
    pub addr: SocketAddr,
    shutdown: Option<oneshot::Sender<()>>,
    task: JoinHandle<()>,
}

impl RunningProxy {
    pub async fn stop(mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
        tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .expect("proxy should stop")
            .unwrap();
    }

    pub fn signal_shutdown(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }

    pub async fn join(self) {
        tokio::time::timeout(Duration::from_secs(5), self.task)
            .await
            .expect("proxy should stop")
            .unwrap();
    }
}

pub async fn spawn_proxy(routes: Routes, config: ProxyConfig) -> RunningProxy {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (tx, rx) = oneshot::channel::<()>();
    let proxy = Proxy::new(routes, config, None);
    let task = tokio::spawn(async move {
        proxy
            .serve(listener, async {
                let _ = rx.await;
            })
            .await;
    });
    RunningProxy {
        addr,
        shutdown: Some(tx),
        task,
    }
}

pub struct Reply {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

impl Reply {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

pub async fn send(proxy: SocketAddr, request: Request<Full<Bytes>>) -> Reply {
    let stream = TcpStream::connect(proxy).await.unwrap();
    send_on(stream, request).await
}

pub async fn send_on<I>(io: I, request: Request<Full<Bytes>>) -> Reply
where
    I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut sender, conn) = hyper::client::conn::http1::handshake(TokioIo::new(io))
        .await
        .unwrap();
    tokio::spawn(conn);
    let response = sender.send_request(request).await.unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        headers,
        body,
    }
}

pub const UPGRADE_REQUEST: &str =
    "GET /ws HTTP/1.1\r\nhost: web.localhost\r\nconnection: Upgrade\r\nupgrade: websocket\r\n\r\n";

pub async fn open_tunnel<I>(io: &mut I, host: &str) -> String
where
    I: AsyncRead + AsyncWrite + Unpin,
{
    let request = UPGRADE_REQUEST.replace("web.localhost", host);
    io.write_all(request.as_bytes()).await.unwrap();
    let mut head = Vec::new();
    let mut byte = [0_u8; 1];
    while !head.ends_with(b"\r\n\r\n") {
        io.read_exact(&mut byte).await.unwrap();
        head.push(byte[0]);
    }
    String::from_utf8(head).unwrap()
}

pub async fn assert_echoes<I: AsyncRead + AsyncWrite + Unpin>(io: &mut I) {
    for message in [&b"ping"[..], b"second message", &[0_u8, 255, 7]] {
        io.write_all(message).await.unwrap();
        let mut echoed = vec![0_u8; message.len()];
        io.read_exact(&mut echoed).await.unwrap();
        assert_eq!(echoed, message);
    }
}

pub async fn get(proxy: SocketAddr, host: &str, path: &str) -> Reply {
    let request = Request::get(path)
        .header("host", host)
        .body(Full::new(Bytes::new()))
        .unwrap();
    send(proxy, request).await
}
