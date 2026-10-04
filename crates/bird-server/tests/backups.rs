mod support;

use reqwest::StatusCode;
use serde_json::json;
use support::{Birdd, SHELL_IMAGE, scope};

const DATA: &str = "/usr/share/nginx/html";

async fn write(birdd: &Birdd, scope: &str, content: &str) {
    let script = format!("echo {content} > {DATA}/kept.txt");
    let (code, output) = birdd
        .exec(&birdd.root, scope, "site", &["sh", "-c", &script])
        .await;
    assert!(code == 0, "writing failed: {output}");
}

async fn read(birdd: &Birdd, scope: &str) -> String {
    let file = format!("{DATA}/kept.txt");
    birdd
        .exec(&birdd.root, scope, "site", &["cat", &file])
        .await
        .1
}

// a restore puts back exactly what the backup copied, and keeps what it replaced as a backup
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn restores_volume_data() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let production = scope("default", "production");
    let service = format!("{production}/services/site");
    let deployed = birdd
        .post(
            &root,
            &format!("{production}/deploy"),
            json!({ "name": "site", "image": SHELL_IMAGE, "port": 80, "volumes": [{ "name": "data", "path": DATA }] }),
        )
        .await;
    assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);

    write(&birdd, &production, "before").await;
    let backup = birdd
        .post(&root, &format!("{service}/backups"), json!(null))
        .await;
    assert_eq!(backup.status, StatusCode::CREATED, "{:?}", backup.body);
    let id = backup.body["id"].as_str().unwrap().to_owned();
    write(&birdd, &production, "after").await;

    let restored = birdd
        .post(&root, &format!("{service}/backups/{id}/restore"), json!({}))
        .await;
    assert_eq!(restored.status, StatusCode::OK, "{:?}", restored.body);
    assert_eq!(read(&birdd, &production).await, "before\n");

    let safety = restored.body["safety_backup"].as_str().unwrap().to_owned();
    let listed = birdd.get(&root, &format!("{service}/backups")).await.body;
    let triggers: Vec<_> = listed
        .as_array()
        .unwrap()
        .iter()
        .map(|b| b["trigger"].clone())
        .collect();
    assert!(triggers.contains(&json!("restore")) && triggers.contains(&json!("manual")));
    let undo = birdd
        .post(
            &root,
            &format!("{service}/backups/{safety}/restore"),
            json!({}),
        )
        .await;
    assert_eq!(undo.status, StatusCode::OK, "{:?}", undo.body);
    assert_eq!(read(&birdd, &production).await, "after\n");

    // backups outlive the service, and a removed service's backups are still listed
    let purged = birdd.delete(&root, &format!("{service}?purge=true")).await;
    assert_eq!(purged.status, StatusCode::NO_CONTENT);
    let kept = birdd.get(&root, &format!("{service}/backups")).await.body;
    assert_eq!(kept.as_array().unwrap().len(), 3);
    birdd.stop().await;
}
