mod support;

use reqwest::StatusCode;
use serde_json::{Value, json};
use support::{Birdd, SHELL_IMAGE, scope};

const SERVE_SECOND: &str =
    "echo second > /usr/share/nginx/html/index.html; exec nginx -g 'daemon off;'";

async fn deploy(birdd: &Birdd, scope: &str, extra: Value) -> support::Response {
    let mut body = json!({ "name": "web", "image": SHELL_IMAGE, "port": 80 });
    if let (Some(body), Some(extra)) = (body.as_object_mut(), extra.as_object()) {
        body.extend(extra.clone());
    }
    birdd
        .post(&birdd.root, &format!("{scope}/deploy"), body)
        .await
}

async fn page(birdd: &Birdd, scope: &str) -> String {
    let index = ["cat", "/usr/share/nginx/html/index.html"];
    birdd.exec(&birdd.root, scope, "web", &index).await.1
}

// a release command sees the new deploy's variables, and failing it leaves the old machines serving
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn release_commands_gate_deploys() {
    let birdd = Birdd::start().await;
    let production = scope("default", "production");
    let service = format!("{production}/services/web");

    let first = deploy(&birdd, &production, json!({})).await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.body);
    let original = page(&birdd, &production).await;

    let failed = deploy(
        &birdd,
        &production,
        json!({
            "env": { "GREETING": "hi" },
            "command": ["sh", "-c", SERVE_SECOND],
            "release": ["sh", "-c", "echo migrating $GREETING; exit 3"]
        }),
    )
    .await;
    assert_eq!(failed.status, StatusCode::BAD_GATEWAY, "{:?}", failed.body);
    assert!(
        failed.body["error"]
            .as_str()
            .unwrap()
            .contains("exited with code 3"),
        "{:?}",
        failed.body
    );
    assert_eq!(failed.body["logs"], json!(["migrating hi"]));
    let status = birdd.get(&birdd.root, &service).await.body;
    assert_eq!(status["deployment"]["id"], first.body["deployment_id"]);
    assert!(status["failed_deploy"].is_object(), "{status}");
    assert_eq!(page(&birdd, &production).await, original);

    // the failed deploy saved neither its release nor its command
    let plain = deploy(&birdd, &production, json!({})).await;
    assert_eq!(plain.status, StatusCode::OK, "{:?}", plain.body);
    assert_eq!(page(&birdd, &production).await, original);

    let checked = json!(["sh", "-c", "test \"$GREETING\" = hi"]);
    let passed = deploy(
        &birdd,
        &production,
        json!({ "command": ["sh", "-c", SERVE_SECOND], "release": checked }),
    )
    .await;
    assert_eq!(passed.status, StatusCode::OK, "{:?}", passed.body);
    assert_eq!(page(&birdd, &production).await, "second\n");

    // the release is kept and runs with the variables this deploy brings
    let changed = deploy(&birdd, &production, json!({ "env": { "GREETING": "bye" } })).await;
    assert_eq!(
        changed.status,
        StatusCode::BAD_GATEWAY,
        "{:?}",
        changed.body
    );
    let status = birdd.get(&birdd.root, &service).await.body;
    assert_eq!(status["deployment"]["id"], passed.body["deployment_id"]);

    let both = deploy(
        &birdd,
        &production,
        json!({ "release": checked, "no_release": true }),
    )
    .await;
    assert_eq!(both.status, StatusCode::BAD_REQUEST, "{:?}", both.body);
    let dropped = deploy(&birdd, &production, json!({ "no_release": true })).await;
    assert_eq!(dropped.status, StatusCode::OK, "{:?}", dropped.body);

    assert_eq!(
        birdd.delete(&birdd.root, &service).await.status,
        StatusCode::NO_CONTENT
    );
    birdd.stop().await;
}
