use std::process::Command;

use bird_podman::{Podman, default_socket};

const SERVER: &str = "docker.io/traefik/whoami:latest";
const CLIENT: &str = "docker.io/library/alpine:3";

fn podman_cli(args: &[&str]) -> (bool, String) {
    let output = Command::new("podman")
        .args(args)
        .output()
        .expect("podman is installed");
    (
        output.status.success(),
        String::from_utf8_lossy(&output.stdout).trim().to_owned(),
    )
}

fn reaches(network: &str, address: &str) -> bool {
    let url = format!("http://{address}/");
    podman_cli(&[
        "run",
        "--rm",
        "--network",
        network,
        CLIENT,
        "wget",
        "-q",
        "-T",
        "3",
        "-O-",
        &url,
    ])
    .0
}

// environments share a host, so a machine in one must not reach another's by address
#[tokio::test]
#[ignore = "requires a running podman socket"]
async fn networks_are_isolated_from_each_other() {
    let podman = Podman::new(default_socket());
    let id = std::process::id();
    let (home, other) = (
        format!("bird-test-home-{id}"),
        format!("bird-test-other-{id}"),
    );
    let server = format!("bird-test-isolated-{id}");
    for network in [&home, &other] {
        podman.ensure_network(network).await.unwrap();
    }
    podman_cli(&["pull", "-q", CLIENT]);
    let (started, _) = podman_cli(&["run", "-d", "--name", &server, "--network", &home, SERVER]);
    assert!(started);
    let (_, address) = podman_cli(&[
        "inspect",
        &server,
        "--format",
        "{{range .NetworkSettings.Networks}}{{.IPAddress}}{{end}}",
    ]);

    let same = reaches(&home, &address);
    let across = reaches(&other, &address);
    podman_cli(&["rm", "-f", &server]);
    for network in [&home, &other] {
        podman.remove_network(network).await.unwrap();
    }
    assert!(same, "a machine reaches its own network");
    assert!(!across, "another network reaches {address}");
}
