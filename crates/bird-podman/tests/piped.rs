use std::collections::BTreeMap;
use std::time::Duration;

use bird_podman::{ContainerSpec, Demux, Lifecycle, Limits, LogStream, Podman, default_socket};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const IMAGE: &str = "docker.io/library/alpine:3";
const COMMAND: &str = "cat; echo read-it-all >&2; exit 7";

fn spec(name: &str, command: &[&str]) -> ContainerSpec {
    ContainerSpec {
        name: name.to_owned(),
        image: IMAGE.parse().expect("a valid image"),
        command: Some(command.iter().map(|arg| (*arg).to_owned()).collect()),
        entrypoint: None,
        lifecycle: Lifecycle::OneOff,
        network: "podman".to_owned(),
        aliases: Vec::new(),
        mounts: Vec::new(),
        limits: Limits {
            memory_bytes: 128 * 1024 * 1024,
            cpu_millicores: 500,
            pids: 128,
        },
        env: BTreeMap::new(),
        labels: BTreeMap::new(),
    }
}

// every byte value, more than fits in one podman frame or one pipe buffer
fn binary_input() -> Vec<u8> {
    (0..=255_u8).cycle().take(3 * 1024 * 1024 + 7).collect()
}

// writes all of `input`, then closes the write side so the command reads end of file
async fn round_trip(io: TokioIo<Upgraded>, input: Vec<u8>) -> (Vec<u8>, Vec<u8>) {
    let (mut read, mut write) = tokio::io::split(io);
    let writer = tokio::spawn(async move {
        write
            .write_all(&input)
            .await
            .expect("podman takes the input");
        write.shutdown().await.expect("the write side closes");
    });
    let mut raw = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), read.read_to_end(&mut raw))
        .await
        .expect("the command sees end of file and exits")
        .expect("the output stays readable");
    writer.await.expect("the writer finishes");
    let (mut stdout, mut stderr) = (Vec::new(), Vec::new());
    for (stream, piece) in Demux::default().push(&raw).expect("framed output") {
        match stream {
            LogStream::Stdout => stdout.extend_from_slice(piece),
            LogStream::Stderr => stderr.extend_from_slice(piece),
        }
    }
    (stdout, stderr)
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn piped_exec_passes_bytes_and_end_of_file() {
    let podman = Podman::new(default_socket());
    podman
        .pull_image(&IMAGE.parse().unwrap(), None)
        .await
        .unwrap();
    let name = "bird-test-piped-exec";
    let _ = podman.remove_container(name).await;
    let id = podman
        .create_container(&spec(name, &["sleep", "60"]))
        .await
        .unwrap();
    podman.start_container(&id).await.unwrap();

    let command = ["sh", "-c", COMMAND].map(String::from);
    let session = podman.exec_piped(&id, &command).await.unwrap();
    let session_id = session.id().to_owned();
    let input = binary_input();
    let (stdout, stderr) = round_trip(session.io, input.clone()).await;
    assert!(stdout == input, "stdout changed: {} bytes", stdout.len());
    assert_eq!(stderr, b"read-it-all\n");
    tokio::time::sleep(Duration::from_millis(300)).await;
    let info = podman.inspect_exec(&session_id).await.unwrap();
    assert_eq!((info.running, info.exit_code), (false, 7));
    podman.remove_container(&id).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn piped_container_passes_bytes_and_end_of_file() {
    let podman = Podman::new(default_socket());
    podman
        .pull_image(&IMAGE.parse().unwrap(), None)
        .await
        .unwrap();
    let name = "bird-test-piped-run";
    let _ = podman.remove_container(name).await;
    let mut piped = spec(name, &["sh", "-c", COMMAND]);
    piped.lifecycle = Lifecycle::Piped;
    let id = podman.create_container(&piped).await.unwrap();
    let io = podman.attach(&id).await.unwrap();
    podman.start_container(&id).await.unwrap();

    let input = binary_input();
    let (stdout, stderr) = round_trip(io, input.clone()).await;
    assert!(stdout == input, "stdout changed: {} bytes", stdout.len());
    assert_eq!(stderr, b"read-it-all\n");
    let code = podman
        .wait_container(&id, Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(code, 7);
    podman.remove_container(&id).await.unwrap();
}
