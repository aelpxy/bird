use std::collections::BTreeMap;
use std::time::Duration;

use bird_core::{ImageRef, Port};
use bird_podman::{ContainerSpec, ContainerState, Error, LogStream, Podman, default_socket};
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
    for _ in 0..50 {
        if let Some(response) = http_get(port).await {
            return Some(response);
        }
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
    None
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn container_lifecycle() {
    let podman = podman();
    podman.ping().await.unwrap();

    let name = format!("bird-test-{}", std::process::id());
    podman.ensure_network(&name).await.unwrap();
    podman.ensure_network(&name).await.unwrap();

    let image: ImageRef = IMAGE.parse().unwrap();
    assert_ne!(podman.pull_image(&image).await.unwrap(), "");
    assert!(podman.image_exists(&image).await.unwrap());

    let port = Port::try_from(80).unwrap();
    let spec = ContainerSpec {
        name: name.clone(),
        image,
        port,
        network: name.clone(),
        aliases: vec!["lifecycle-alias".to_owned()],
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

    let logs = podman.logs(&id, 100).await.unwrap();
    assert!(
        logs.iter()
            .any(|l| l.stream == LogStream::Stdout && l.text.contains("GET /")),
        "{logs:?}"
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
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn pulling_missing_image_fails() {
    let image: ImageRef = "bird-does-not-exist-xyz:latest".parse().unwrap();
    assert!(matches!(
        podman().pull_image(&image).await.unwrap_err(),
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
