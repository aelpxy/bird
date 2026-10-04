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

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn tty_exec_is_interactive_and_resizable() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let podman = podman();
    podman
        .pull_image(&IMAGE.parse().unwrap(), None)
        .await
        .unwrap();
    let name = "bird-test-tty";
    let _ = podman.remove_container(name).await;
    let id = podman
        .create_container(&spec(name, "sleep 60"))
        .await
        .unwrap();
    podman.start_container(&id).await.unwrap();

    let mut session = podman.exec_tty(&id, &["sh".to_owned()]).await.unwrap();
    podman.resize_exec(session.id(), 132, 40).await.unwrap();
    session
        .io
        .write_all(b"stty size; echo $((6 * 7)); exit 4\n")
        .await
        .unwrap();
    let mut output = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), session.io.read_to_end(&mut output))
        .await
        .unwrap()
        .unwrap();
    let output = String::from_utf8_lossy(&output);
    assert!(output.contains("40 132"), "{output}");
    assert!(output.contains("42"), "{output}");
    let info = podman.inspect_exec(session.id()).await.unwrap();
    assert_eq!((info.running, info.exit_code), (false, 4));

    // podman keeps a terminal session running when its client leaves, so callers must hang it up
    let abandoned = podman
        .exec_tty(&id, &["sleep".to_owned(), "300".to_owned()])
        .await
        .unwrap();
    let abandoned_id = abandoned.id().to_owned();
    drop(abandoned);
    tokio::time::sleep(Duration::from_secs(1)).await;
    let info = podman.inspect_exec(&abandoned_id).await.unwrap();
    assert!(info.running);
    assert!(info.pid.is_some());
    podman.remove_container(&id).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn terminal_container_is_attached_from_the_start() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    let podman = podman();
    podman
        .pull_image(&IMAGE.parse().unwrap(), None)
        .await
        .unwrap();
    let name = "bird-test-terminal";
    let _ = podman.remove_container(name).await;
    let mut terminal = spec(name, "unused");
    terminal.lifecycle = Lifecycle::Terminal;
    terminal.command = Some(vec!["sh".to_owned()]);
    let id = podman.create_container(&terminal).await.unwrap();
    let mut io = podman.attach(&id).await.unwrap();
    podman.start_container(&id).await.unwrap();
    podman.resize_container(&id, 111, 33).await.unwrap();
    // busybox asks the terminal for the cursor position before its prompt, so type after the prompt
    let mut early = Vec::new();
    let mut chunk = [0_u8; 1024];
    tokio::time::timeout(Duration::from_secs(10), async {
        while !String::from_utf8_lossy(&early).contains("# ") {
            let read = io.read(&mut chunk).await.unwrap();
            early.extend_from_slice(&chunk[..read]);
        }
    })
    .await
    .unwrap();
    io.write_all(b"echo greet=$GREETING; stty size; exit 6\n")
        .await
        .unwrap();
    let mut output = Vec::new();
    tokio::time::timeout(Duration::from_secs(10), io.read_to_end(&mut output))
        .await
        .unwrap()
        .unwrap();
    let output = String::from_utf8_lossy(&output);
    assert!(output.contains("greet=hi"), "{output}");
    assert!(output.contains("33 111"), "{output}");
    let code = podman
        .wait_container(&id, Duration::from_secs(10))
        .await
        .unwrap();
    assert_eq!(code, 6);
    podman.remove_container(&id).await.unwrap();
}
