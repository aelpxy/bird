mod support;

use std::time::Duration;

use bird_proxy::ProxyConfig;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, StatusCode};
use support::{raw_exchange, routes, send, spawn_proxy, spawn_upstream};

fn limited() -> ProxyConfig {
    ProxyConfig {
        max_body_bytes: 1024,
        body_idle_timeout: Duration::from_millis(200),
        ..ProxyConfig::default()
    }
}

#[tokio::test]
async fn rejects_declared_oversized_bodies() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(routes(&[("web.localhost", &[upstream])]), limited()).await;
    for (size, status) in [
        (1024, StatusCode::OK),
        (1025, StatusCode::PAYLOAD_TOO_LARGE),
    ] {
        let request = Request::post("/count")
            .header("host", "web.localhost")
            .body(Full::new(Bytes::from(vec![b'x'; size])))
            .unwrap();
        assert_eq!(send(proxy.addr, request).await.status, status, "{size}");
    }
    proxy.stop().await;
}

#[tokio::test]
async fn stops_chunked_bodies_at_the_limit() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(routes(&[("web.localhost", &[upstream])]), limited()).await;
    let mut request = b"POST /count HTTP/1.1\r\nhost: web.localhost\r\nconnection: close\r\ntransfer-encoding: chunked\r\n\r\n".to_vec();
    for _ in 0..4 {
        request.extend_from_slice(b"200\r\n");
        request.extend_from_slice(&[b'x'; 512]);
        request.extend_from_slice(b"\r\n");
    }
    request.extend_from_slice(b"0\r\n\r\n");
    let reply = raw_exchange(proxy.addr, &request).await;
    assert!(reply.starts_with("HTTP/1.1 413"), "{reply}");
    proxy.stop().await;
}

#[tokio::test]
async fn closes_responses_that_stall() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(routes(&[("web.localhost", &[upstream])]), limited()).await;
    let reply = raw_exchange(
        proxy.addr,
        b"GET /stall HTTP/1.1\r\nhost: web.localhost\r\nconnection: close\r\n\r\n",
    )
    .await;
    assert!(reply.starts_with("HTTP/1.1 200"), "{reply}");
    assert!(reply.contains("partial"), "{reply}");
    assert!(!reply.ends_with("0\r\n\r\n"), "{reply}");
    proxy.stop().await;
}
