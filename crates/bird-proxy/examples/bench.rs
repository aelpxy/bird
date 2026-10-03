//! Proxy benchmark harness. Run the two halves as separate processes, then load both ports:
//!
//! ```text
//! cargo run --release -p bird-proxy --example bench -- upstream   # 127.0.0.1:9101
//! cargo run --release -p bird-proxy --example bench -- proxy      # 127.0.0.1:9100
//! oha -z 10s -c 64 http://127.0.0.1:9101/                         # baseline
//! oha -z 10s -c 64 -H 'Host: bench.localhost' http://127.0.0.1:9100/
//! ```

use std::convert::Infallible;
use std::net::SocketAddr;

use bird_proxy::{Proxy, ProxyConfig, RouteTable, Routes};
use bytes::Bytes;
use http_body_util::Full;
use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper::{Request, Response};
use hyper_util::rt::TokioIo;
use tokio::net::TcpListener;

const UPSTREAM: &str = "127.0.0.1:9101";
const PROXY: &str = "127.0.0.1:9100";
const BODY: &[u8] = b"hello from the bench upstream\n";

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    match std::env::args().nth(1).as_deref() {
        Some("upstream") => upstream().await,
        Some("proxy") => proxy().await,
        _ => Err("usage: bench <upstream|proxy>".into()),
    }
}

async fn upstream() -> Result<(), Box<dyn std::error::Error>> {
    let listener = TcpListener::bind(UPSTREAM).await?;
    println!("upstream on {UPSTREAM}");
    loop {
        let (stream, _) = listener.accept().await?;
        stream.set_nodelay(true)?;
        tokio::spawn(async move {
            let service = service_fn(|_: Request<hyper::body::Incoming>| async {
                Ok::<_, Infallible>(Response::new(Full::new(Bytes::from_static(BODY))))
            });
            let _ = http1::Builder::new()
                .serve_connection(TokioIo::new(stream), service)
                .await;
        });
    }
}

async fn proxy() -> Result<(), Box<dyn std::error::Error>> {
    let upstream: SocketAddr = UPSTREAM.parse()?;
    let mut table = RouteTable::new();
    table.insert(&"bench.localhost".parse()?, &[upstream]);
    let routes = Routes::new();
    routes.replace(table);

    let listener = TcpListener::bind(PROXY).await?;
    println!("proxy on {PROXY} -> {UPSTREAM} for Host: bench.localhost");
    Proxy::new(routes, ProxyConfig::default())
        .serve(listener, async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await;
    Ok(())
}
