mod support;

use reqwest::StatusCode;
use reqwest::header::{HOST, LOCATION};
use serde_json::json;
use support::{Birdd, IMAGE, scope};

const APP: &str = "web.localhost";
const WWW: &str = "www.web.localhost";

async fn visit(birdd: &Birdd, host: &str) -> reqwest::Response {
    reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("an http client")
        .get(format!("http://{}/", birdd.proxy))
        .header(HOST, host)
        .send()
        .await
        .expect("the proxy answers")
}

// what a user sets in the dashboard reaches the proxy on the next request
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn proxy_settings_apply_to_the_next_request() {
    let birdd = Birdd::start().await;
    let production = scope("default", "production");
    let service = format!("{production}/services/web");
    let deployed = birdd
        .post(
            &birdd.root,
            &format!("{production}/deploy"),
            json!({ "name": "web", "image": IMAGE, "port": 80, "domains": [APP, WWW] }),
        )
        .await;
    assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);

    let served = visit(&birdd, APP).await;
    assert_eq!(served.status(), StatusCode::OK);
    let machine = served.headers()["x-machine-id"]
        .to_str()
        .unwrap()
        .to_owned();
    let detail = birdd.get(&birdd.root, &service).await;
    let machines = detail.body["deployment"]["machines"].as_array().unwrap();
    assert!(
        machines.iter().any(|m| m["id"] == machine.as_str()),
        "{machine} not in {machines:?}"
    );

    let settings = json!({
        "rate_limit": { "per_second": 1, "burst": 1 },
        "headers": { "cache-control": "no-store" }
    });
    let saved = birdd
        .put(&birdd.root, &format!("{service}/proxy"), settings.clone())
        .await;
    assert_eq!(saved.status, StatusCode::OK, "{:?}", saved.body);
    let read = birdd.get(&birdd.root, &format!("{service}/proxy")).await;
    assert_eq!(read.body["rate_limit"], settings["rate_limit"]);
    let first = visit(&birdd, APP).await;
    assert_eq!(first.headers()["cache-control"], "no-store");
    assert_eq!(
        visit(&birdd, APP).await.status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    let maintenance = birdd
        .put(
            &birdd.root,
            &format!("{service}/proxy"),
            json!({ "maintenance": true }),
        )
        .await;
    assert_eq!(maintenance.status, StatusCode::OK, "{:?}", maintenance.body);
    assert_eq!(
        visit(&birdd, APP).await.status(),
        StatusCode::SERVICE_UNAVAILABLE
    );

    let bad = birdd
        .put(
            &birdd.root,
            &format!("{service}/proxy"),
            json!({ "headers": { "set-cookie": "a=b" } }),
        )
        .await;
    assert_eq!(
        bad.status,
        StatusCode::UNPROCESSABLE_ENTITY,
        "{:?}",
        bad.body
    );

    let redirected = birdd
        .put(
            &birdd.root,
            &format!("{service}/domains/{WWW}"),
            json!({ "redirect_to": APP }),
        )
        .await;
    assert_eq!(
        redirected.status,
        StatusCode::NO_CONTENT,
        "{:?}",
        redirected.body
    );
    let moved = visit(&birdd, WWW).await;
    assert_eq!(moved.status(), StatusCode::PERMANENT_REDIRECT);
    assert_eq!(moved.headers()[LOCATION], "http://web.localhost/");
    let listed = birdd.get(&birdd.root, &format!("{service}/domains")).await;
    let www = listed
        .body
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["hostname"] == WWW)
        .unwrap();
    assert_eq!(www["redirect_to"], APP);
    assert_eq!(www["https"]["state"], "local");

    birdd.stop().await;
}
