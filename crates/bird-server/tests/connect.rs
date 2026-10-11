mod support;

use std::time::Duration;

use reqwest::StatusCode;
use serde_json::json;
use support::{Birdd, SHELL_IMAGE, scope};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const QUIET: Duration = Duration::from_secs(3);

// writes `request` on a fresh connection to birdd and reads until the other side closes
async fn exchange(birdd: &Birdd, path: &str, then: &[u8]) -> String {
    let mut stream = TcpStream::connect(&birdd.api).await.expect("birdd listens");
    let head = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nAuthorization: Bearer {}\r\nConnection: upgrade\r\nUpgrade: bird-tcp\r\n\r\n",
        birdd.api, birdd.root
    );
    stream
        .write_all(head.as_bytes())
        .await
        .expect("request sent");
    stream.write_all(then).await.expect("payload sent");
    // birdd keeps an errored connection open and a tunnel closes when the app does, so read
    // until the other side closes or goes quiet
    let mut answer = Vec::new();
    let mut chunk = [0; 4096];
    while let Ok(Ok(read)) = tokio::time::timeout(QUIET, stream.read(&mut chunk)).await {
        if read == 0 {
            break;
        }
        answer.extend_from_slice(chunk.get(..read).unwrap_or_default());
    }
    String::from_utf8_lossy(&answer).into_owned()
}

// `bird connect` carries a client's bytes to the service's port and back
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn tunnels_reach_the_services_port() {
    let birdd = Birdd::start().await;
    let production = scope("default", "production");
    let deployed = birdd
        .deploy(&birdd.root, &production, "web", SHELL_IMAGE)
        .await;
    assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);

    let path = format!("{production}/services/web/connect");
    let answer = exchange(
        &birdd,
        &path,
        b"GET / HTTP/1.0\r\nHost: web\r\nConnection: close\r\n\r\n",
    )
    .await;
    assert!(answer.starts_with("HTTP/1.1 101"), "{answer}");
    let tunnelled = answer.split("\r\n\r\n").nth(1).unwrap_or_default();
    assert!(tunnelled.starts_with("HTTP/1.1 200"), "{answer}");
    assert!(tunnelled.contains("nginx"), "{answer}");

    let plain = birdd.get(&birdd.root, &path).await;
    assert_eq!(
        plain.status,
        StatusCode::UPGRADE_REQUIRED,
        "{:?}",
        plain.body
    );
    let missing = exchange(&birdd, &format!("{production}/services/nope/connect"), b"").await;
    assert!(missing.starts_with("HTTP/1.1 404"), "{missing}");

    birdd
        .post(
            &birdd.root,
            &format!("{production}/services/web/stop"),
            json!({}),
        )
        .await;
    let stopped = exchange(&birdd, &path, b"").await;
    assert!(stopped.starts_with("HTTP/1.1 404"), "{stopped}");

    birdd.stop().await;
}
