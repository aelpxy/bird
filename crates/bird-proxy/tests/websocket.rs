mod support;

use std::time::Duration;

use bird_proxy::ProxyConfig;
use support::tls::{SECURE, spawn_edge, tls_stream};
use support::{assert_echoes, get, open_tunnel, routes, spawn_proxy, spawn_upstream};
use tokio::io::AsyncReadExt;
use tokio::net::TcpStream;

#[tokio::test]
async fn tunnels_websocket_upgrades() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;

    let mut stream = TcpStream::connect(proxy.addr).await.unwrap();
    let head = open_tunnel(&mut stream, "web.localhost")
        .await
        .to_ascii_lowercase();
    assert!(head.starts_with("http/1.1 101"), "{head}");
    assert!(head.contains("upgrade: websocket"), "{head}");
    assert!(head.contains("connection: upgrade"), "{head}");
    assert_echoes(&mut stream).await;
    proxy.stop().await;
}

#[tokio::test]
async fn tunnels_websockets_over_tls() {
    let upstream = spawn_upstream("a").await;
    let edge = spawn_edge(routes(&[(SECURE, &[upstream])]));
    let mut stream = tls_stream(&edge, SECURE).await.unwrap();
    let head = open_tunnel(&mut stream, SECURE).await.to_ascii_lowercase();
    assert!(head.starts_with("http/1.1 101"), "{head}");
    assert_echoes(&mut stream).await;
}

#[tokio::test]
async fn shutdown_closes_open_tunnels() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;
    let mut stream = TcpStream::connect(proxy.addr).await.unwrap();
    open_tunnel(&mut stream, "web.localhost").await;
    assert_echoes(&mut stream).await;

    tokio::time::timeout(Duration::from_secs(2), proxy.stop())
        .await
        .expect("shutdown should not wait for the open tunnel");
    let mut rest = Vec::new();
    let read = tokio::time::timeout(Duration::from_secs(2), stream.read_to_end(&mut rest)).await;
    assert!(
        matches!(read, Ok(Ok(0) | Err(_))),
        "tunnel should be closed: {read:?}"
    );
}

#[tokio::test]
async fn tunnels_count_against_the_connection_limit() {
    let upstream = spawn_upstream("a").await;
    let config = ProxyConfig {
        max_connections: 1,
        ..ProxyConfig::default()
    };
    let proxy = spawn_proxy(routes(&[("web.localhost", &[upstream])]), config).await;
    let mut stream = TcpStream::connect(proxy.addr).await.unwrap();
    open_tunnel(&mut stream, "web.localhost").await;
    assert_echoes(&mut stream).await;

    let blocked = tokio::time::timeout(
        Duration::from_millis(300),
        get(proxy.addr, "web.localhost", "/"),
    )
    .await;
    assert!(
        blocked.is_err(),
        "a second connection was served while the tunnel was open"
    );

    drop(stream);
    let reply = tokio::time::timeout(
        Duration::from_secs(2),
        get(proxy.addr, "web.localhost", "/"),
    )
    .await
    .expect("the slot should free up once the tunnel closes");
    assert_eq!(reply.status, hyper::StatusCode::OK);
    proxy.stop().await;
}
