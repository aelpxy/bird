use std::collections::BTreeMap;
use std::time::Duration;

use bird_podman::{ContainerSpec, Lifecycle, Limits, Podman, default_socket};

const IMAGE: &str = "docker.io/library/alpine:3";

fn spec(name: &str) -> ContainerSpec {
    ContainerSpec {
        name: name.to_owned(),
        image: IMAGE.parse().expect("a valid image"),
        command: Some(
            ["sh", "-c", "while :; do :; done"]
                .map(String::from)
                .to_vec(),
        ),
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

#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn busy_container_measures_its_cpu_limit() {
    let podman = Podman::new(default_socket());
    podman
        .pull_image(&IMAGE.parse().unwrap(), None)
        .await
        .unwrap();
    let name = "bird-test-stats";
    let _ = podman.remove_container(name).await;
    let id = podman.create_container(&spec(name)).await.unwrap();
    podman.start_container(&id).await.unwrap();

    let first = podman.stats(&[&id]).await.unwrap()[&id];
    tokio::time::sleep(Duration::from_secs(1)).await;
    let second = podman.stats(&[&id]).await.unwrap()[&id];
    let cpu = second.cpu_millicores_since(&first).unwrap();
    assert!((400..=600).contains(&cpu), "{cpu}");
    assert!(second.memory_bytes > 0);
    assert!(second.processes >= 1);

    assert!(podman.stats(&[]).await.unwrap().is_empty());
    podman.remove_container(&id).await.unwrap();
    assert!(podman.stats(&[&id]).await.is_err());
}
