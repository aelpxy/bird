mod support;

use hyper::StatusCode;
use hyper::header::LOCATION;
use support::tls::{PLAIN, SECURE, https_get, spawn_edge};
use support::{get, routes, spawn_upstream};

#[tokio::test]
async fn serves_https_with_the_matching_certificate() {
    let upstream = spawn_upstream("a").await;
    let edge = spawn_edge(routes(&[(SECURE, &[upstream])]));

    let reply = https_get(&edge, SECURE, "/hello").await.unwrap();
    assert_eq!(reply.status, StatusCode::OK);
    let body = reply.text();
    assert!(body.contains("uri=/hello"), "{body}");
    assert!(body.contains("proto=https"), "{body}");
}

#[tokio::test]
async fn refuses_handshake_for_unknown_names() {
    let upstream = spawn_upstream("a").await;
    let edge = spawn_edge(routes(&[(SECURE, &[upstream])]));
    assert!(https_get(&edge, "other.localhost", "/").await.is_err());
}

#[tokio::test]
async fn redirects_http_only_for_hosts_with_certificates() {
    let upstream = spawn_upstream("a").await;
    let edge = spawn_edge(routes(&[(SECURE, &[upstream]), (PLAIN, &[upstream])]));

    let redirected = get(edge.http, SECURE, "/path?q=1").await;
    assert_eq!(redirected.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(
        redirected.headers[LOCATION],
        format!("https://{SECURE}:{}/path?q=1", edge.https.port()).as_str()
    );

    let proxied = get(edge.http, PLAIN, "/").await;
    assert_eq!(proxied.status, StatusCode::OK);
    assert!(proxied.text().contains("proto=http\n"));
}

#[tokio::test]
async fn answers_acme_challenges_over_http() {
    let edge = spawn_edge(routes(&[]));
    edge.challenges
        .insert("token123".to_owned(), "token123.thumbprint".to_owned());

    let reply = get(
        edge.http,
        "new.example.com",
        "/.well-known/acme-challenge/token123",
    )
    .await;
    assert_eq!(reply.status, StatusCode::OK);
    assert_eq!(reply.text(), "token123.thumbprint");

    let unknown = get(
        edge.http,
        "new.example.com",
        "/.well-known/acme-challenge/nope",
    )
    .await;
    assert_eq!(unknown.status, StatusCode::NOT_FOUND);
}
