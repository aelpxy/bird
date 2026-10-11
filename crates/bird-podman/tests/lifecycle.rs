use std::collections::BTreeMap;
use std::time::Duration;

use bird_core::{ImageRef, Port};
use bird_podman::{
    ContainerSpec, ContainerState, Error, Lifecycle, Limits, LogStream, Podman, VolumeMount,
    default_socket,
};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

const IMAGE: &str = "nginx:alpine";

fn podman() -> Podman {
    Podman::new(default_socket())
}

async fn http_get(port: u16) -> Option<String> {
    let mut stream = TcpStream::connect(("127.0.0.1", port)).await.ok()?;
    stream
        .write_all(b"GET / HTTP/1.0\r\nHost: test\r\n\r\n")
        .await
        .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).await.ok()?;
    Some(response)
}

async fn wait_for_http(port: u16) -> Option<String> {
    for _ in 0..300 {
        if let Some(response) = http_get(port).await {
            return Some(response);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

// nginx logs a request after answering it, so the line can trail the response
async fn wait_for_log(podman: &Podman, id: &str, text: &str) -> Vec<bird_podman::LogLine> {
    let mut logs = Vec::new();
    for _ in 0..50 {
        logs = podman.logs(id, 100).await.expect("logs stay readable");
        if logs.iter().any(|l| l.text.contains(text)) {
            break;
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    logs
}

// a time after every line leaves none, one before them all keeps them
async fn assert_since_filters(podman: &Podman, id: &str) {
    let future = std::time::SystemTime::now() + Duration::from_secs(3600);
    let none_yet = podman
        .recent_logs(id, 100, Some(future))
        .await
        .expect("logs stay readable");
    assert!(none_yet.is_empty(), "{none_yet:?}");
    let since_start = podman
        .recent_logs(id, 100, Some(std::time::UNIX_EPOCH))
        .await
        .expect("logs stay readable");
    assert!(since_start.iter().any(|l| l.text.contains("GET /")));
}

// every line carries when podman recorded it, and none still starts with podman's own prefix
fn assert_timed(logs: &[bird_podman::LogLine]) {
    let now_ms = i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the clock is after 1970")
            .as_millis(),
    )
    .expect("milliseconds since 1970 fit in an i64");
    for line in logs {
        let at = line.at.expect("podman was asked for timestamps");
        assert!((now_ms - at).abs() < 120_000, "{line:?}");
        let prefix = line.text.get(..20).unwrap_or_default();
        assert!(
            !(prefix.get(4..5) == Some("-") && prefix.get(10..11) == Some("T")),
            "{line:?}"
        );
    }
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn container_lifecycle() {
    let podman = podman();
    podman.ping().await.unwrap();

    let name = format!("bird-test-{}", std::process::id());
    podman.ensure_network(&name).await.unwrap();
    podman.ensure_network(&name).await.unwrap();
    let labels = BTreeMap::from([("bird.test".to_owned(), name.clone())]);
    podman.ensure_volume(&name, &labels).await.unwrap();
    podman.ensure_volume(&name, &labels).await.unwrap();

    let image: ImageRef = IMAGE.parse().unwrap();
    assert_ne!(podman.pull_image(&image, None).await.unwrap(), "");
    assert!(podman.image_exists(&image).await.unwrap());

    let port = Port::try_from(80).unwrap();
    let spec = ContainerSpec {
        name: name.clone(),
        image,
        command: None,
        entrypoint: None,
        lifecycle: Lifecycle::Service { port },
        network: name.clone(),
        aliases: vec!["lifecycle-alias".to_owned()],
        limits: Limits {
            memory_bytes: 256 * 1024 * 1024,
            cpu_millicores: 500,
            pids: 512,
        },
        mounts: vec![VolumeMount {
            volume: name.clone(),
            destination: "/usr/share/nginx/html/data".to_owned(),
        }],
        env: BTreeMap::from([("GREETING".parse().unwrap(), "hi".to_owned())]),
        labels: BTreeMap::from([("bird.test".to_owned(), name.clone())]),
    };
    let id = podman.create_container(&spec).await.unwrap();
    assert!(matches!(
        podman.create_container(&spec).await.unwrap_err(),
        Error::Conflict { .. }
    ));

    podman.start_container(&id).await.unwrap();
    podman.start_container(&id).await.unwrap();

    let info = podman.inspect_container(&id).await.unwrap();
    assert_eq!(info.state, ContainerState::Running);
    assert!(
        info.aliases.contains(&"lifecycle-alias".to_owned()),
        "{:?}",
        info.aliases
    );
    let host_port = info.host_port(port).expect("port should be published");
    let response = wait_for_http(host_port)
        .await
        .expect("container should answer http");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");

    let listed = podman
        .list_containers(&format!("bird.test={name}"))
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].host_port(port), Some(host_port));

    let logs = wait_for_log(&podman, &id, "GET /").await;
    assert!(
        logs.iter()
            .any(|l| l.stream == LogStream::Stdout && l.text.contains("GET /")),
        "{logs:?}"
    );
    assert_timed(&logs);
    assert_since_filters(&podman, &id).await;

    podman.pause_container(&id).await.unwrap();
    assert_eq!(
        podman.inspect_container(&id).await.unwrap().state,
        ContainerState::Paused
    );
    podman.unpause_container(&id).await.unwrap();
    assert_eq!(
        podman.inspect_container(&id).await.unwrap().state,
        ContainerState::Running
    );

    podman
        .stop_container(&id, Duration::from_secs(5))
        .await
        .unwrap();
    podman
        .stop_container(&id, Duration::from_secs(5))
        .await
        .unwrap();
    assert_eq!(
        podman.inspect_container(&id).await.unwrap().state,
        ContainerState::Exited
    );

    podman.remove_container(&id).await.unwrap();
    assert!(matches!(
        podman.inspect_container(&id).await.unwrap_err(),
        Error::NotFound { .. }
    ));
    podman.remove_network(&name).await.unwrap();
    podman.remove_volume(&name).await.unwrap();
    assert!(matches!(
        podman.remove_volume(&name).await.unwrap_err(),
        Error::NotFound { .. }
    ));
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn pulling_missing_image_fails() {
    let image: ImageRef = "bird-does-not-exist-xyz:latest".parse().unwrap();
    assert!(matches!(
        podman().remove_image(&image).await.unwrap_err(),
        Error::NotFound { .. }
    ));
    assert!(matches!(
        podman().pull_image(&image, None).await.unwrap_err(),
        Error::Pull { .. }
    ));
}

#[tokio::test]
async fn unreachable_socket_is_reported() {
    let podman = Podman::new("/nonexistent/podman.sock");
    assert!(matches!(
        podman.ping().await.unwrap_err(),
        Error::Connect { .. }
    ));
}
