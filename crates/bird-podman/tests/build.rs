use std::collections::BTreeMap;

use bird_core::ImageRef;
use bird_podman::{BuildLine, Podman, default_socket};
use bytes::Bytes;
use http_body_util::Full;

fn podman() -> Podman {
    Podman::new(default_socket())
}

fn context(dockerfile: &str) -> Full<Bytes> {
    let mut builder = tar::Builder::new(Vec::new());
    for (name, content) in [("Dockerfile", dockerfile), ("hello.txt", "hi\n")] {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();
        builder
            .append_data(&mut header, name, content.as_bytes())
            .expect("writing to a Vec cannot fail");
    }
    Full::new(Bytes::from(
        builder.into_inner().expect("writing to a Vec cannot fail"),
    ))
}

async fn lines(podman: &Podman, dockerfile: &str, tag: &ImageRef) -> Vec<BuildLine> {
    let args = BTreeMap::from([(
        "GREETING".parse().expect("a valid key"),
        "hi & bye".to_owned(),
    )]);
    let mut output = podman
        .build_image(context(dockerfile), tag, "Dockerfile", &args)
        .await
        .expect("podman accepts the build");
    let mut lines = Vec::new();
    while let Some(line) = output.next().await {
        lines.push(line.expect("the build output stays readable"));
    }
    lines
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn builds_and_tags_an_image() {
    let podman = podman();
    let tag: ImageRef = "localhost/bird-test/build:ok".parse().unwrap();
    let dockerfile = "FROM docker.io/library/alpine:3\nARG GREETING\nRUN echo \"got $GREETING\"\nCOPY hello.txt /hello.txt\n";
    let lines = lines(&podman, dockerfile, &tag).await;
    assert!(
        !lines.iter().any(|l| matches!(l, BuildLine::Failed(_))),
        "{lines:?}"
    );
    assert!(
        lines.contains(&BuildLine::Log("got hi & bye".to_owned())),
        "{lines:?}"
    );
    assert!(podman.image_exists(&tag).await.unwrap());
    podman.remove_image(&tag).await.unwrap();
}

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn reports_a_failing_step() {
    let podman = podman();
    let tag: ImageRef = "localhost/bird-test/build:fail".parse().unwrap();
    let dockerfile = "FROM docker.io/library/alpine:3\nRUN echo boom && exit 7\n";
    let lines = lines(&podman, dockerfile, &tag).await;
    assert!(
        lines.contains(&BuildLine::Log("boom".to_owned())),
        "{lines:?}"
    );
    assert!(matches!(lines.last(), Some(BuildLine::Failed(e)) if e.contains("exit status 7")));
    assert!(!podman.image_exists(&tag).await.unwrap());
}
