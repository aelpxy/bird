mod support;

use reqwest::StatusCode;
use serde_json::json;
use support::{Birdd, IMAGE, SHELL_IMAGE, scope};

// where `api` resolves to from the environment's probe
async fn address(birdd: &Birdd, scope: &str) -> String {
    let (code, output) = birdd
        .exec(&birdd.root, scope, "probe", &["getent", "hosts", "api"])
        .await;
    assert!(code == 0, "lookup failed: {output}");
    output
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_owned()
}

// the probe's exit code fetching from an address, 0 when it got an answer
async fn fetch(birdd: &Birdd, scope: &str, target: &str) -> i32 {
    let url = format!("http://{target}/");
    let wget = ["wget", "-q", "-T", "3", "-O-", url.as_str()];
    birdd.exec(&birdd.root, scope, "probe", &wget).await.0
}

// two environments of one project run the same names on their own networks, and neither can
// reach the other by name or by address
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
#[allow(
    clippy::too_many_lines,
    reason = "one scenario reads best top to bottom"
)]
async fn environments_are_isolated() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let project = birdd.project("shop");
    assert_eq!(
        birdd
            .post(&root, "/v1/projects", json!({ "name": project }))
            .await
            .status,
        StatusCode::CREATED
    );
    let staging_created = birdd
        .post(
            &root,
            &format!("/v1/projects/{project}/environments"),
            json!({ "name": "staging" }),
        )
        .await;
    assert_eq!(staging_created.status, StatusCode::CREATED);
    let (production, staging) = (scope(&project, "production"), scope(&project, "staging"));

    for environment in [&production, &staging] {
        for (name, image) in [("api", IMAGE), ("probe", SHELL_IMAGE)] {
            let deployed = birdd.deploy(&root, environment, name, image).await;
            assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);
        }
    }
    let (production_api, staging_api) = (
        address(&birdd, &production).await,
        address(&birdd, &staging).await,
    );
    assert_ne!(production_api, staging_api);

    assert_eq!(fetch(&birdd, &production, &production_api).await, 0);
    assert_ne!(
        fetch(&birdd, &production, &staging_api).await,
        0,
        "production reached staging"
    );
    assert_ne!(
        fetch(&birdd, &staging, &production_api).await,
        0,
        "staging reached production"
    );

    // references resolve inside the environment only
    let only_production = birdd.deploy(&root, &production, "only-here", IMAGE).await;
    assert_eq!(only_production.status, StatusCode::OK);
    let reference = birdd
        .call(
            &root,
            reqwest::Method::PATCH,
            &format!("{staging}/services/probe/variables"),
            Some(json!({ "set": { "X": "${{only-here.HOSTNAME}}" }, "deploy": false })),
        )
        .await;
    assert_eq!(
        reference.status,
        StatusCode::BAD_REQUEST,
        "{:?}",
        reference.body
    );

    // an environment with services is not deleted, an empty one takes its network with it
    let refused = birdd
        .delete(
            &root,
            &format!("/v1/projects/{project}/environments/staging"),
        )
        .await;
    assert_eq!(refused.status, StatusCode::CONFLICT);
    for name in ["api", "probe"] {
        assert_eq!(
            birdd
                .delete(&root, &format!("{staging}/services/{name}"))
                .await
                .status,
            StatusCode::NO_CONTENT
        );
    }
    let deleted = birdd
        .delete(
            &root,
            &format!("/v1/projects/{project}/environments/staging"),
        )
        .await;
    assert_eq!(deleted.status, StatusCode::NO_CONTENT);
    let networks = std::process::Command::new("podman")
        .args(["network", "ls", "--format", "{{.Name}}"])
        .output()
        .unwrap();
    let networks = String::from_utf8_lossy(&networks.stdout);
    assert!(
        !networks
            .lines()
            .any(|name| name == format!("bird_{project}_staging"))
    );
    assert!(
        networks
            .lines()
            .any(|name| name == format!("bird_{project}_production"))
    );
    birdd.stop().await;
}
