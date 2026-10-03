mod support;

use bird_proxy::ProxyConfig;
use bytes::Bytes;
use http_body_util::Full;
use hyper::{Request, StatusCode};
use support::{get, routes, send, spawn_proxy, spawn_upstream, table};

#[tokio::test]
async fn forwards_request_to_upstream() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;

    let reply = get(proxy.addr, "web.localhost:8080", "/hello?x=1").await;
    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.text();
    assert!(body.contains("upstream=a"), "{body}");
    assert!(body.contains("method=GET"), "{body}");
    assert!(body.contains("uri=/hello?x=1"), "{body}");
    assert!(body.contains("host=web.localhost:8080"), "{body}");
    assert!(body.contains("xff=127.0.0.1"), "{body}");
    assert!(body.contains("xfh=web.localhost:8080"), "{body}");
    assert!(body.contains("proto=http"), "{body}");
    proxy.stop().await;
}

#[tokio::test]
async fn host_matching_is_case_insensitive() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;
    assert_eq!(
        get(proxy.addr, "WEB.Localhost.", "/").await.status,
        StatusCode::OK
    );
    proxy.stop().await;
}

#[tokio::test]
async fn strips_hop_by_hop_and_spoofed_headers() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;

    let request = Request::get("/")
        .header("host", "web.localhost")
        .header("connection", "x-secret")
        .header("x-secret", "leak")
        .header("x-forwarded-for", "6.6.6.6")
        .body(Full::new(Bytes::new()))
        .unwrap();
    let body = send(proxy.addr, request).await.text();
    assert!(body.contains("secret=-"), "{body}");
    assert!(body.contains("xff=127.0.0.1\n"), "{body}");
    proxy.stop().await;
}

#[tokio::test]
async fn forwards_the_host_it_routed_on() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;

    let request = Request::get("http://web.localhost/x")
        .header("host", "evil.example")
        .body(Full::new(Bytes::new()))
        .unwrap();
    let body = send(proxy.addr, request).await.text();
    assert!(body.contains("host=web.localhost\n"), "{body}");
    assert!(body.contains("xfh=web.localhost\n"), "{body}");
    proxy.stop().await;
}

#[tokio::test]
async fn balances_across_machines() {
    let a = spawn_upstream("a").await;
    let b = spawn_upstream("b").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[a, b])]),
        ProxyConfig::default(),
    )
    .await;

    let mut seen = Vec::new();
    for _ in 0..4 {
        let body = get(proxy.addr, "web.localhost", "/").await.text();
        seen.push(body.lines().next().unwrap_or_default().to_owned());
    }
    assert_eq!(
        seen,
        ["upstream=a", "upstream=b", "upstream=a", "upstream=b"]
    );
    proxy.stop().await;
}

#[tokio::test]
async fn route_changes_apply_without_restart() {
    let a = spawn_upstream("a").await;
    let b = spawn_upstream("b").await;
    let routes = routes(&[("web.localhost", &[a])]);
    let proxy = spawn_proxy(routes.clone(), ProxyConfig::default()).await;

    assert!(
        get(proxy.addr, "web.localhost", "/")
            .await
            .text()
            .starts_with("upstream=a")
    );
    routes.replace(table(&[("web.localhost", &[b])]));
    assert!(
        get(proxy.addr, "web.localhost", "/")
            .await
            .text()
            .starts_with("upstream=b")
    );
    proxy.stop().await;
}

#[tokio::test]
async fn streams_large_bodies() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;

    let payload: Vec<u8> = (0..8 * 1024 * 1024_u32)
        .map(|i| u8::try_from(i % 251).unwrap())
        .collect();
    let request = Request::post("/echo")
        .header("host", "web.localhost")
        .body(Full::new(Bytes::from(payload.clone())))
        .unwrap();
    let reply = send(proxy.addr, request).await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.body.len(), payload.len());
    let first_mismatch = reply.body.iter().zip(&payload).position(|(a, b)| a != b);
    assert_eq!(first_mismatch, None);
    proxy.stop().await;
}

#[tokio::test]
async fn marks_responses_as_served_by_bird() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;
    for path in ["/", "/branded"] {
        let reply = get(proxy.addr, "web.localhost", path).await;
        let values: Vec<_> = reply.headers.get_all("x-server").iter().collect();
        assert_eq!(values, ["Bird"], "{path}");
    }
    proxy.stop().await;
}
