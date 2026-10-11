mod support;

use std::net::SocketAddr;
use std::sync::Arc;

use bird_core::{Hostname, MachineId};
use bird_proxy::{ProxyConfig, Route, RoutePolicy, RouteTable, Routes};
use bytes::Bytes;
use http_body_util::Full;
use hyper::header::{LOCATION, RETRY_AFTER};
use hyper::{Request, StatusCode};
use support::{get, send, spawn_proxy, spawn_upstream};

const APP: &str = "app.localhost";

fn host(name: &str) -> Hostname {
    name.parse().expect("a valid hostname")
}

fn app(upstream: SocketAddr, machine: MachineId, settings: &str) -> Routes {
    let settings = serde_json::from_str(settings).expect("valid proxy settings");
    let mut table = RouteTable::new();
    table.insert_route(
        &host(APP),
        Route {
            upstreams: vec![(upstream, Some(machine))],
            policy: Some(Arc::new(RoutePolicy::new(&settings))),
            redirect_to: None,
        },
    );
    table.insert_route(
        &host("www.app.localhost"),
        Route {
            redirect_to: Some(host(APP)),
            ..Route::default()
        },
    );
    let routes = Routes::new();
    routes.replace(table);
    routes
}

#[tokio::test]
async fn names_the_machine_and_sets_the_apps_headers() {
    let upstream = spawn_upstream("a").await;
    let machine = MachineId::generate();
    let settings = r#"{"security_headers":true,"headers":{"cache-control":"no-store"}}"#;
    let proxy = spawn_proxy(app(upstream, machine, settings), ProxyConfig::default()).await;

    let reply = get(proxy.addr, APP, "/").await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.headers["x-machine-id"], machine.to_string().as_str());
    assert_eq!(reply.headers["cache-control"], "no-store");
    assert_eq!(reply.headers["x-content-type-options"], "nosniff");
    proxy.stop().await;
}

#[tokio::test]
async fn limits_each_visitor_with_retry_after() {
    let upstream = spawn_upstream("a").await;
    let settings = r#"{"rate_limit":{"per_second":1,"burst":2}}"#;
    let proxy = spawn_proxy(
        app(upstream, MachineId::generate(), settings),
        ProxyConfig::default(),
    )
    .await;

    assert_eq!(get(proxy.addr, APP, "/").await.status, StatusCode::OK);
    assert_eq!(get(proxy.addr, APP, "/").await.status, StatusCode::OK);
    let limited = get(proxy.addr, APP, "/").await;
    assert_eq!(limited.status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(limited.headers[RETRY_AFTER], "1");
    proxy.stop().await;
}

#[tokio::test]
async fn caps_the_apps_bodies_and_answers_for_it_in_maintenance() {
    let upstream = spawn_upstream("a").await;
    let capped = spawn_proxy(
        app(upstream, MachineId::generate(), r#"{"max_body_mb":1}"#),
        ProxyConfig::default(),
    )
    .await;
    let big = Request::post("/")
        .header("host", APP)
        .body(Full::new(Bytes::from(vec![b'x'; 2 * 1024 * 1024])))
        .unwrap();
    assert_eq!(
        send(capped.addr, big).await.status,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    capped.stop().await;

    let closed = spawn_proxy(
        app(upstream, MachineId::generate(), r#"{"maintenance":true}"#),
        ProxyConfig::default(),
    )
    .await;
    let reply = get(closed.addr, APP, "/").await;
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(reply.text().contains("maintenance"), "{}", reply.text());
    closed.stop().await;
}

#[tokio::test]
async fn redirects_a_domain_to_another_of_the_app() {
    let upstream = spawn_upstream("a").await;
    let proxy = spawn_proxy(
        app(upstream, MachineId::generate(), "{}"),
        ProxyConfig::default(),
    )
    .await;
    let reply = get(proxy.addr, "www.app.localhost", "/shop?item=1").await;
    assert_eq!(reply.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(reply.headers[LOCATION], "http://app.localhost/shop?item=1");
    proxy.stop().await;
}
