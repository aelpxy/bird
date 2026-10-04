use std::collections::BTreeMap;
use std::time::Duration;

use bird_podman::{ContainerSpec, Lifecycle, Limits, LogLine, LogStream, Podman, default_socket};

const IMAGE: &str = "docker.io/library/alpine:3";

fn podman() -> Podman {
    Podman::new(default_socket())
}

fn spec(name: &str, command: &str) -> ContainerSpec {
    ContainerSpec {
        name: name.to_owned(),
        image: IMAGE.parse().expect("a valid image"),
        command: Some(vec!["sh".to_owned(), "-c".to_owned(), command.to_owned()]),
        lifecycle: Lifecycle::OneOff,
        network: "podman".to_owned(),
        aliases: Vec::new(),
        mounts: Vec::new(),
        limits: Limits {
            memory_bytes: 128 * 1024 * 1024,
            cpu_millicores: 500,
            pids: 128,
        },
        env: BTreeMap::from([("GREETING".parse().expect("a valid key"), "hi".to_owned())]),
        labels: BTreeMap::new(),
    }
}

fn line(stream: LogStream, text: &str) -> LogLine {
    LogLine {
        stream,
        text: text.to_owned(),
    }
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn exec_streams_output_and_exit_code() {
    let podman = podman();
    podman
        .pull_image(&IMAGE.parse().unwrap(), None)
        .await
        .unwrap();
    let name = "bird-test-exec";
    let _ = podman.remove_container(name).await;
    let id = podman
        .create_container(&spec(name, "sleep 60"))
        .await
        .unwrap();
    podman.start_container(&id).await.unwrap();

    let command = ["sh", "-c", "echo $GREETING; echo oops >&2; exit 3"].map(String::from);
    let mut session = podman.exec(&id, &command).await.unwrap();
    let mut lines = Vec::new();
    while let Some(next) = session.output.next().await {
        lines.push(next.unwrap());
    }
    assert!(lines.contains(&line(LogStream::Stdout, "hi")), "{lines:?}");
    assert!(
        lines.contains(&line(LogStream::Stderr, "oops")),
        "{lines:?}"
    );
    assert_eq!(podman.exec_exit_code(&session).await.unwrap(), 3);

    let missing = podman.exec(&id, &["nosuchcmd".to_owned()]).await.unwrap();
    let mut session = missing;
    while session.output.next().await.is_some() {}
    assert_eq!(podman.exec_exit_code(&session).await.unwrap(), 127);
    podman.remove_container(&id).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn one_off_container_reports_output_and_exit_code() {
    let podman = podman();
    podman
        .pull_image(&IMAGE.parse().unwrap(), None)
        .await
        .unwrap();
    let name = "bird-test-one-off";
    let _ = podman.remove_container(name).await;
    let id = podman
        .create_container(&spec(name, "echo start; sleep 1; echo done >&2; exit 5"))
        .await
        .unwrap();
    podman.start_container(&id).await.unwrap();

    let mut output = podman.follow_output(&id).await.unwrap();
    let mut lines = Vec::new();
    while let Some(next) = output.next().await {
        lines.push(next.unwrap());
    }
    assert_eq!(
        lines,
        vec![
            line(LogStream::Stdout, "start"),
            line(LogStream::Stderr, "done")
        ]
    );
    let code = podman
        .wait_container(&id, Duration::from_secs(30))
        .await
        .unwrap();
    assert_eq!(code, 5);
    let info = podman.inspect_container(&id).await.unwrap();
    assert_eq!(info.ports, Vec::new());
    podman.remove_container(&id).await.unwrap();
}
