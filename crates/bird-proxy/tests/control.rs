mod support;

use std::net::SocketAddr;
use std::time::Duration;

use bird_proxy::{ControlRoute, ProxyConfig};
use hyper::StatusCode;
use hyper::header::LOCATION;
use support::tls::{SECURE, spawn_edge_with};
use support::{get, routes, spawn_proxy, spawn_upstream};

const CONTROL: &str = "bird.example.com";

fn control(upstream: SocketAddr) -> ControlRoute {
    ControlRoute {
        host: CONTROL.parse().expect("a valid hostname"),
        upstream,
        response_timeout: Duration::from_secs(10),
        body_idle_timeout: Duration::from_secs(10),
        max_body_bytes: 1024,
    }
}

#[tokio::test]
async fn the_control_host_has_its_own_upstream_and_limits() {
    let app = spawn_upstream("app").await;
    let api = spawn_upstream("api").await;
    let config = ProxyConfig {
        response_timeout: Duration::from_millis(100),
        control: Some(control(api)),
        ..ProxyConfig::default()
    };
    let proxy = spawn_proxy(routes(&[("web.localhost", &[app])]), config).await;

    let slow = get(proxy.addr, CONTROL, "/slow").await;
    assert_eq!(slow.status, StatusCode::OK);
    assert!(slow.text().contains("upstream=api\n"), "{}", slow.text());
    let app_slow = get(proxy.addr, "web.localhost", "/slow").await;
    assert_eq!(app_slow.status, StatusCode::GATEWAY_TIMEOUT);
    proxy.stop().await;
}

#[tokio::test]
async fn the_control_host_redirects_to_https_before_it_has_a_certificate() {
    let api = spawn_upstream("api").await;
    let config = ProxyConfig {
        control: Some(control(api)),
        ..ProxyConfig::default()
    };
    let edge = spawn_edge_with(routes(&[(SECURE, &[api])]), config);

    let reply = get(edge.http, CONTROL, "/v1/login").await;
    assert_eq!(reply.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        reply.headers[LOCATION],
        format!("https://{CONTROL}:{}/v1/login", edge.https.port()).as_str()
    );
}
