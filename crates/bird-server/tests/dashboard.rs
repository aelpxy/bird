mod support;

use reqwest::StatusCode;
use reqwest::header::{CACHE_CONTROL, CONTENT_TYPE, HeaderMap};
use support::Birdd;

async fn fetch(birdd: &Birdd, path: &str) -> (StatusCode, HeaderMap, String) {
    let response = reqwest::get(format!("http://{}{path}", birdd.api))
        .await
        .expect("birdd answers");
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.text().await.expect("a text body");
    (status, headers, body)
}

// needs `pnpm build` in frontend/ before birdd is built, as CI does
#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn serves_the_dashboard_and_its_routes_without_a_token() {
    let birdd = Birdd::start().await;

    let (status, headers, index) = fetch(&birdd, "/").await;
    assert_eq!(status, StatusCode::OK, "{index}");
    assert_eq!(headers[CONTENT_TYPE], "text/html; charset=utf-8");
    assert_eq!(headers[CACHE_CONTROL], "no-cache");
    assert_eq!(headers["content-security-policy"], "frame-ancestors 'none'");
    assert!(index.contains(r#"<div id="root">"#), "{index}");

    let (status, _, page) = fetch(&birdd, "/projects/prod/production").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page, index);

    let script = index
        .split(r#"src=""#)
        .nth(1)
        .and_then(|rest| rest.split('"').next())
        .expect("index.html loads a script");
    let (status, headers, _) = fetch(&birdd, script).await;
    assert_eq!(status, StatusCode::OK, "{script}");
    assert_eq!(headers[CONTENT_TYPE], "text/javascript; charset=utf-8");
    assert_eq!(
        headers[CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );

    for missing in ["/assets/gone-123.js", "/v1/nope"] {
        assert_eq!(
            fetch(&birdd, missing).await.0,
            StatusCode::NOT_FOUND,
            "{missing}"
        );
    }
    let (status, _, _) = fetch(&birdd, "/v1/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    birdd.stop().await;
}
