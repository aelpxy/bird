mod support;

use std::time::Duration;

use bird_proxy::ProxyConfig;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, StatusCode};
use support::{get, routes, send, spawn_proxy, spawn_upstream};
use tokio::net::TcpListener;

#[tokio::test]
async fn unknown_host_is_not_found() {
    let proxy = spawn_proxy(routes(&[]), ProxyConfig::default()).await;
    let reply = get(proxy.addr, "nope.localhost", "/").await;
    assert_eq!(reply.status, StatusCode::NOT_FOUND);
    assert_eq!(reply.headers["x-server"], "Bird");
    proxy.stop().await;
}

#[tokio::test]
async fn missing_host_is_bad_request() {
    let proxy = spawn_proxy(routes(&[]), ProxyConfig::default()).await;
    let request = Request::get("/").body(Full::new(Bytes::new())).unwrap();
    assert_eq!(
        send(proxy.addr, request).await.status,
        StatusCode::BAD_REQUEST
    );
    proxy.stop().await;
}

#[tokio::test]
async fn app_without_machines_is_unavailable() {
    let proxy = spawn_proxy(routes(&[("web.localhost", &[])]), ProxyConfig::default()).await;
    let reply = get(proxy.addr, "web.localhost", "/").await;
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    proxy.stop().await;
}

#[tokio::test]
async fn dead_upstream_is_bad_gateway() {
    let dead = {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        listener.local_addr().unwrap()
    };
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[dead])]),
        ProxyConfig::default(),
    )
    .await;
    let reply = get(proxy.addr, "web.localhost", "/").await;
    assert_eq!(reply.status, StatusCode::BAD_GATEWAY);
    proxy.stop().await;
}

#[tokio::test]
async fn slow_upstream_times_out() {
    let upstream = spawn_upstream("a").await;
    let config = ProxyConfig {
        response_timeout: Duration::from_millis(100),
        ..ProxyConfig::default()
    };
    let proxy = spawn_proxy(routes(&[("web.localhost", &[upstream])]), config).await;
    let reply = get(proxy.addr, "web.localhost", "/slow").await;
    assert_eq!(reply.status, StatusCode::GATEWAY_TIMEOUT);
    proxy.stop().await;
}
