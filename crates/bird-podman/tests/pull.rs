use std::net::TcpListener;
use std::time::{Duration, Instant};

use bird_core::ImageRef;
use bird_podman::{Error, Podman, default_socket};

// a registry that is down at first must not fail the pull on the first try
#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn failed_pulls_are_retried() {
    let podman = Podman::new(default_socket());
    let port = TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .unwrap()
        .port();
    let image: ImageRef = format!("127.0.0.1:{port}/bird-test/missing:1")
        .parse()
        .unwrap();

    let started = Instant::now();
    let err = podman.pull_image(&image, None).await.unwrap_err();
    assert!(matches!(err, Error::Pull { .. }), "{err}");
    // two waits between three attempts, 2s then 4s
    assert!(
        started.elapsed() >= Duration::from_secs(6),
        "{:?}",
        started.elapsed()
    );
}
