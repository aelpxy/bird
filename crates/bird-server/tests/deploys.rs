mod support;

use reqwest::StatusCode;
use serde_json::{Value, json};
use support::{Birdd, SHELL_IMAGE, scope};

// a failed deploy leaves the running one serving, and a rollback brings back an earlier image
async fn page(birdd: &Birdd, scope: &str) -> String {
    let index = ["cat", "/usr/share/nginx/html/index.html"];
    birdd.exec(&birdd.root, scope, "web", &index).await.1
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn failed_deploys_keep_serving_and_rollbacks_restore() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let production = scope("default", "production");
    let service = format!("{production}/services/web");

    let first = birdd.deploy(&root, &production, "web", SHELL_IMAGE).await;
    assert_eq!(first.status, StatusCode::OK, "{:?}", first.body);
    let marked = birdd
        .post(
            &root,
            &format!("{production}/deploy"),
            json!({
                "name": "web", "image": SHELL_IMAGE, "port": 80,
                "command": ["sh", "-c", "echo second > /usr/share/nginx/html/index.html; exec nginx -g 'daemon off;'"]
            }),
        )
        .await;
    assert_eq!(marked.status, StatusCode::OK, "{:?}", marked.body);
    assert_eq!(page(&birdd, &production).await, "second\n");

    // the image exits at once, so it never passes its health check
    let broken = birdd
        .post(
            &root,
            &format!("{production}/deploy"),
            json!({ "name": "web", "image": SHELL_IMAGE, "port": 80, "command": ["false"] }),
        )
        .await;
    assert_eq!(broken.status, StatusCode::BAD_GATEWAY, "{:?}", broken.body);
    let status = birdd.get(&root, &service).await.body;
    assert_eq!(status["deployment"]["id"], marked.body["deployment_id"]);
    assert!(status["failed_deploy"].is_object(), "{status}");
    assert_eq!(page(&birdd, &production).await, "second\n");

    // the command is kept for later deploys until asked to drop it
    let kept = birdd.deploy(&root, &production, "web", SHELL_IMAGE).await;
    assert_eq!(kept.status, StatusCode::OK);
    assert_eq!(page(&birdd, &production).await, "second\n");
    let both = birdd
        .post(
            &root,
            &format!("{production}/deploy"),
            json!({ "name": "web", "image": SHELL_IMAGE, "port": 80, "command": ["sh"], "default_command": true }),
        )
        .await;
    assert_eq!(both.status, StatusCode::BAD_REQUEST);
    let reset = birdd
        .post(
            &root,
            &format!("{production}/deploy"),
            json!({ "name": "web", "image": SHELL_IMAGE, "port": 80, "default_command": true }),
        )
        .await;
    assert_eq!(reset.status, StatusCode::OK, "{:?}", reset.body);
    assert_ne!(page(&birdd, &production).await, "second\n");

    // rolling back restores the command the earlier deployment ran with
    let rolled = birdd
        .post(
            &root,
            &format!("{service}/rollback"),
            json!({ "deployment_id": marked.body["deployment_id"] }),
        )
        .await;
    assert_eq!(rolled.status, StatusCode::OK, "{:?}", rolled.body);
    assert_eq!(page(&birdd, &production).await, "second\n");
    let history = birdd
        .get(&root, &format!("{service}/deployments"))
        .await
        .body;
    let statuses: Vec<&Value> = history
        .as_array()
        .unwrap()
        .iter()
        .map(|d| &d["status"])
        .collect();
    assert_eq!(statuses.iter().filter(|s| **s == "active").count(), 1);
    assert!(statuses.contains(&&json!("failed")));

    assert_eq!(
        birdd.delete(&root, &service).await.status,
        StatusCode::NO_CONTENT
    );
    birdd.stop().await;
}
