mod support;

use std::time::{SystemTime, UNIX_EPOCH};

use reqwest::StatusCode;
use ring::hmac;
use serde_json::{Value, json};
use support::Birdd;

const PASSWORD: &str = "correct horse battery";

// what an authenticator app shows for a base32 secret, `steps` periods from now
fn code(secret: &str, steps: u64) -> String {
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567";
    let (mut key, mut buffer, mut bits) = (Vec::new(), 0_u64, 0_u32);
    for symbol in secret.bytes() {
        let value = alphabet
            .iter()
            .position(|known| *known == symbol)
            .expect("birdd hands out base32 secrets");
        buffer = (buffer << 5) | u64::try_from(value).expect("an index below 32");
        bits += 5;
        if bits >= 8 {
            bits -= 8;
            key.extend_from_slice(&((buffer >> bits) & 0xff).to_be_bytes()[7..]);
        }
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("the clock is past 1970")
        .as_secs();
    let counter = (now / 30 + steps).to_be_bytes();
    let tag = hmac::sign(
        &hmac::Key::new(hmac::HMAC_SHA1_FOR_LEGACY_USE_ONLY, &key),
        &counter,
    );
    let digest = tag.as_ref();
    let offset = usize::from(digest.last().copied().unwrap_or_default() & 0x0f);
    let word = digest
        .get(offset..offset + 4)
        .and_then(|bytes| <[u8; 4]>::try_from(bytes).ok())
        .map_or(0, u32::from_be_bytes)
        & 0x7fff_ffff;
    format!("{:06}", word % 1_000_000)
}

async fn login(
    birdd: &Birdd,
    user: &str,
    password: &str,
    code: Option<&str>,
) -> (StatusCode, Value) {
    let body = json!({ "username": user, "password": password, "code": code });
    let response = birdd.post("", "/v1/login", body).await;
    (response.status, response.body)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
#[allow(
    clippy::too_many_lines,
    reason = "one scenario reads best top to bottom"
)]
async fn passwords_two_factor_and_sessions() {
    let birdd = Birdd::start().await;
    let root = birdd.root.clone();
    let token = birdd.user("ada", "member").await;
    birdd.user("bob", "member").await;

    // no password yet: signing in fails like a wrong password would
    assert_eq!(
        login(&birdd, "ada", PASSWORD, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    let short = birdd
        .put(&token, "/v1/users/ada/password", json!({ "new": "short" }))
        .await;
    assert_eq!(short.status, StatusCode::BAD_REQUEST);
    let set = birdd
        .put(&token, "/v1/users/ada/password", json!({ "new": PASSWORD }))
        .await;
    assert_eq!(set.status, StatusCode::NO_CONTENT, "{:?}", set.body);
    let others = birdd
        .put(&token, "/v1/users/bob/password", json!({ "new": PASSWORD }))
        .await;
    assert_eq!(others.status, StatusCode::FORBIDDEN);

    let (status, body) = login(&birdd, "ada", PASSWORD, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "signed_in");
    let first = body["token"].as_str().unwrap().to_owned();
    assert!(first.starts_with("birds_"));
    assert_eq!(birdd.get(&first, "/v1/me").await.body["has_password"], true);
    assert_eq!(
        login(&birdd, "ada", "wrong password here", None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        login(&birdd, "nobody", PASSWORD, None).await.0,
        StatusCode::UNAUTHORIZED
    );

    // two-factor: a pending secret changes nothing until a code confirms it
    let setup = birdd
        .post(&first, "/v1/users/ada/two-factor", json!(null))
        .await;
    assert_eq!(setup.status, StatusCode::OK);
    let secret = setup.body["secret"].as_str().unwrap().to_owned();
    assert!(
        setup.body["uri"]
            .as_str()
            .unwrap()
            .starts_with("otpauth://totp/bird:ada?")
    );
    let wrong = birdd
        .post(
            &first,
            "/v1/users/ada/two-factor/confirm",
            json!({ "code": "000000" }),
        )
        .await;
    assert_eq!(wrong.status, StatusCode::UNAUTHORIZED);
    let confirm_code = code(&secret, 0);
    let confirmed = birdd
        .post(
            &first,
            "/v1/users/ada/two-factor/confirm",
            json!({ "code": confirm_code }),
        )
        .await;
    assert_eq!(confirmed.status, StatusCode::OK, "{:?}", confirmed.body);
    let recovery: Vec<String> = serde_json::from_value(confirmed.body["codes"].clone()).unwrap();
    assert_eq!(recovery.len(), 10);
    let by_root = birdd
        .post(&root, "/v1/users/ada/two-factor", json!(null))
        .await;
    assert_eq!(by_root.status, StatusCode::FORBIDDEN);

    let (status, body) = login(&birdd, "ada", PASSWORD, None).await;
    assert_eq!(
        (status, body["status"].clone()),
        (StatusCode::OK, json!("two_factor_required"))
    );
    // the code used to confirm does not sign in again, the next one does, once
    assert_eq!(
        login(&birdd, "ada", PASSWORD, Some(&confirm_code)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let next = code(&secret, 1);
    let (status, body) = login(&birdd, "ada", PASSWORD, Some(&next)).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second = body["token"].as_str().unwrap().to_owned();
    assert_eq!(
        login(&birdd, "ada", PASSWORD, Some(&next)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let spaced = recovery[0].to_uppercase().replace('-', " ");
    assert_eq!(
        login(&birdd, "ada", PASSWORD, Some(&spaced)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        login(&birdd, "ada", PASSWORD, Some(&recovery[0])).await.0,
        StatusCode::UNAUTHORIZED
    );

    // sessions: listed with the current one marked, and signing out ends only that one
    let sessions = birdd.get(&second, "/v1/users/ada/sessions").await.body;
    let sessions = sessions.as_array().unwrap();
    assert_eq!(sessions.len(), 3);
    assert_eq!(sessions.iter().filter(|s| s["current"] == true).count(), 1);
    assert_eq!(
        birdd.post(&token, "/v1/logout", json!(null)).await.status,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        birdd.post(&first, "/v1/logout", json!(null)).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd.get(&first, "/v1/me").await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(birdd.get(&second, "/v1/me").await.status, StatusCode::OK);

    // changing your password needs the current one and signs out your other sessions
    let no_current = birdd
        .put(
            &second,
            "/v1/users/ada/password",
            json!({ "new": "another long password" }),
        )
        .await;
    assert_eq!(no_current.status, StatusCode::FORBIDDEN);
    let changed = birdd
        .put(
            &second,
            "/v1/users/ada/password",
            json!({ "current": PASSWORD, "new": "another long password" }),
        )
        .await;
    assert_eq!(changed.status, StatusCode::NO_CONTENT);
    assert_eq!(birdd.get(&second, "/v1/me").await.status, StatusCode::OK);
    assert_eq!(
        birdd
            .get(&second, "/v1/users/ada/sessions")
            .await
            .body
            .as_array()
            .unwrap()
            .len(),
        1
    );

    // an admin resets: everyone's sessions end, two-factor goes off, api tokens keep working
    assert_eq!(
        birdd
            .put(&root, "/v1/users/ada/password", json!({ "new": PASSWORD }))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        birdd.get(&second, "/v1/me").await.status,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        birdd
            .post(&root, "/v1/users/ada/two-factor/disable", json!({}))
            .await
            .status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        login(&birdd, "ada", PASSWORD, None).await.1["status"],
        "signed_in"
    );
    assert_eq!(birdd.get(&token, "/v1/me").await.body["two_factor"], false);
    birdd.stop().await;
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn guessing_slows_down() {
    let birdd = Birdd::start().await;
    let token = birdd.user("ada", "member").await;
    birdd
        .put(&token, "/v1/users/ada/password", json!({ "new": PASSWORD }))
        .await;
    for _ in 0..5 {
        assert_eq!(
            login(&birdd, "ada", "not the password!", None).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let (status, body) = login(&birdd, "ada", PASSWORD, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert!(body["error"].as_str().unwrap().contains("try again in"));
    birdd.stop().await;
}
