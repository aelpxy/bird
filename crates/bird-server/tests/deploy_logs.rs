mod support;

use reqwest::StatusCode;
use reqwest::header::ACCEPT;
use serde_json::{Value, json};
use support::{Birdd, SHELL_IMAGE, scope};

const SERVE: &str = "echo app is booting; exec nginx -g 'daemon off;'";

// a deploy asked for a stream tells what it does as it does it, and keeps that log afterwards
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn deploys_stream_their_log_and_keep_it() {
    let birdd = Birdd::start().await;
    let production = scope("default", "production");
    let request = json!({
        "name": "web",
        "image": SHELL_IMAGE,
        "port": 80,
        "command": ["sh", "-c", SERVE],
        "release": ["sh", "-c", "echo migrating the database"]
    });
    let response = reqwest::Client::new()
        .post(format!("http://{}{production}/deploy", birdd.api))
        .bearer_auth(&birdd.root)
        .header(ACCEPT, "application/x-ndjson")
        .json(&request)
        .send()
        .await
        .expect("birdd answers");
    assert_eq!(response.status(), StatusCode::OK);
    let text = response.text().await.expect("a stream");
    let events: Vec<Value> = text
        .lines()
        .map(|line| serde_json::from_str(line).expect("one json event per line"))
        .collect();

    let last = events.last().expect("at least one event");
    assert_eq!(last["type"], "deployed", "{text}");
    let deployment = last["deployment_id"]
        .as_str()
        .expect("a deployment id")
        .to_owned();
    let logged = |kind: &str, needle: &str| {
        events.iter().any(|event| {
            event["type"] == "log"
                && event["kind"] == kind
                && event["text"]
                    .as_str()
                    .is_some_and(|text| text.contains(needle))
        })
    };
    assert!(logged("bird", "Pulling"), "{text}");
    assert!(logged("release", "migrating the database"), "{text}");
    assert!(logged("app", "app is booting"), "{text}");
    assert!(logged("bird", "is healthy"), "{text}");
    assert!(logged("bird", "Deployed in"), "{text}");

    let saved = birdd
        .get(
            &birdd.root,
            &format!("{production}/services/web/deployments/{deployment}/log"),
        )
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.body);
    assert_eq!(saved.body["done"], true);
    let lines = saved.body["lines"].as_array().expect("lines");
    assert_eq!(lines.len(), events.len() - 1, "{:?}", saved.body);
    assert_eq!(lines[0]["seq"], 0);

    // without the header a deploy answers once, as before
    let plain = birdd
        .post(&birdd.root, &format!("{production}/deploy"), request)
        .await;
    assert_eq!(plain.status, StatusCode::OK, "{:?}", plain.body);
    assert!(plain.body["deployment_id"].is_string());

    birdd.stop().await;
}
