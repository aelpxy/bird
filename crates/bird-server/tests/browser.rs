mod support;

use std::time::Duration;

use bird_api::Frame;
use futures_util::{SinkExt, StreamExt};
use reqwest::StatusCode;
use reqwest::header::{COOKIE, ORIGIN, SET_COOKIE};
use serde_json::{Value, json};
use support::{Birdd, SHELL_IMAGE, scope};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::{self, Message};

const PASSWORD: &str = "correct horse battery";
const TERMINAL_WAIT: Duration = Duration::from_secs(30);

// an admin who signs in with a password, as someone using the dashboard would
async fn admin(birdd: &Birdd) -> String {
    let token = birdd.user("ada", "admin").await;
    let set = birdd
        .put(&token, "/v1/users/ada/password", json!({ "new": PASSWORD }))
        .await;
    assert_eq!(set.status, StatusCode::NO_CONTENT, "{:?}", set.body);
    let signed_in = reqwest::Client::new()
        .post(format!("http://{}/v1/login", birdd.api))
        .json(&json!({ "username": "ada", "password": PASSWORD, "cookie": true }))
        .send()
        .await
        .expect("birdd answers");
    assert_eq!(signed_in.status(), StatusCode::OK);
    let cookie = signed_in
        .headers()
        .get(SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .expect("signing in with a cookie sets one")
        .to_owned();
    let body: Value = signed_in.json().await.expect("a json body");
    assert_eq!(
        body.get("status"),
        Some(&json!("signed_in_with_cookie")),
        "{body}"
    );
    assert!(body.get("token").is_none(), "{body}");
    assert!(cookie.contains("HttpOnly") && cookie.contains("SameSite=Strict"));
    // the test talks to birdd over loopback, where browsers keep cookies on plain http
    assert!(!cookie.contains("Secure"), "{cookie}");
    cookie
        .split(';')
        .next()
        .expect("a cookie starts with its value")
        .to_owned()
}

async fn send(
    birdd: &Birdd,
    method: reqwest::Method,
    path: &str,
    cookie: &str,
    origin: Option<&str>,
) -> reqwest::Response {
    let mut request = reqwest::Client::new()
        .request(method, format!("http://{}{path}", birdd.api))
        .header(COOKIE, cookie);
    if let Some(origin) = origin {
        request = request.header(ORIGIN, origin);
    }
    request.send().await.expect("birdd answers")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn cookie_sessions_only_work_from_this_host() {
    let birdd = Birdd::start().await;
    let cookie = admin(&birdd).await;
    let own = format!("http://{}", birdd.api);
    let get = reqwest::Method::GET;
    let post = reqwest::Method::POST;

    let me = send(&birdd, get.clone(), "/v1/me", &cookie, None).await;
    assert_eq!(me.status(), StatusCode::OK);

    // another site's page, or a user's app on a sibling domain, cannot act with the cookie
    let elsewhere = send(&birdd, post.clone(), "/v1/logout", &cookie, None).await;
    assert_eq!(elsewhere.status(), StatusCode::FORBIDDEN);
    let sibling = Some("http://app.localhost:7070");
    let forged = send(&birdd, post.clone(), "/v1/logout", &cookie, sibling).await;
    assert_eq!(forged.status(), StatusCode::FORBIDDEN);

    // the cookie carries sessions only, never api tokens
    let root = format!("bird_session={}", birdd.root);
    let as_root = send(&birdd, get.clone(), "/v1/me", &root, None).await;
    assert_eq!(as_root.status(), StatusCode::UNAUTHORIZED);

    let out = send(&birdd, post, "/v1/logout", &cookie, Some(&own)).await;
    assert_eq!(out.status(), StatusCode::NO_CONTENT);
    let cleared = out
        .headers()
        .get(SET_COOKIE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default();
    assert!(cleared.contains("Max-Age=0"), "{cleared}");
    let after = send(&birdd, get, "/v1/me", &cookie, None).await;
    assert_eq!(after.status(), StatusCode::UNAUTHORIZED);
    birdd.stop().await;
}

async fn open_terminal(
    birdd: &Birdd,
    path: &str,
    cookie: &str,
    origin: Option<&str>,
) -> Result<tokio_tungstenite::WebSocketStream<TcpStream>, Box<tokio_tungstenite::tungstenite::Error>>
{
    let mut request = format!("ws://{}{path}", birdd.api)
        .into_client_request()
        .expect("a valid websocket url");
    let headers = request.headers_mut();
    headers.insert(COOKIE, cookie.parse().expect("a valid cookie header"));
    if let Some(origin) = origin {
        headers.insert(ORIGIN, origin.parse().expect("a valid origin header"));
    }
    let stream = TcpStream::connect(&birdd.api).await.expect("birdd listens");
    tokio_tungstenite::client_async(request, stream)
        .await
        .map(|(socket, _)| socket)
        .map_err(Box::new)
}

// what the terminal printed and how it ended
async fn session(
    socket: &mut tokio_tungstenite::WebSocketStream<TcpStream>,
    input: &[u8],
) -> (String, Frame) {
    let typed = Frame::Data(input.to_vec()).encode();
    socket
        .send(Message::Binary(typed.into()))
        .await
        .expect("the terminal takes input");
    let (mut output, mut buffer) = (Vec::new(), Vec::new());
    let read = async {
        while let Some(message) = socket.next().await {
            let Message::Binary(bytes) = message.expect("a readable message") else {
                continue;
            };
            buffer.extend_from_slice(&bytes);
            while let Some(frame) = Frame::decode(&mut buffer).expect("well-formed frames") {
                match frame {
                    Frame::Data(bytes) => output.extend_from_slice(&bytes),
                    // anything else ends the session, and the test's assertion names it
                    other => return other,
                }
            }
        }
        Frame::Error("the socket closed before the terminal ended".to_owned())
    };
    let ended = tokio::time::timeout(TERMINAL_WAIT, read)
        .await
        .expect("the terminal ends in time");
    (String::from_utf8_lossy(&output).into_owned(), ended)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "requires a running podman socket"]
async fn browsers_open_terminals_over_websockets() {
    let birdd = Birdd::start().await;
    let cookie = admin(&birdd).await;
    let own = format!("http://{}", birdd.api);
    let production = scope("default", "production");
    let deployed = birdd
        .deploy(&birdd.root, &production, "web", SHELL_IMAGE)
        .await;
    assert_eq!(deployed.status, StatusCode::OK, "{:?}", deployed.body);
    // command=["sh"], as a browser would encode it
    let shell = "command=%5B%22sh%22%5D&cols=80&rows=24";

    let exec = format!("{production}/services/web/exec/tty?{shell}");
    let mut socket = open_terminal(&birdd, &exec, &cookie, Some(&own))
        .await
        .expect("the terminal opens");
    let (output, ended) = session(&mut socket, b"echo $((6 * 7)); exit 3\n").await;
    assert!(output.contains("42"), "{output:?}");
    assert_eq!(ended, Frame::Exit(3));

    let run = format!("{production}/services/web/run/tty?{shell}");
    let mut socket = open_terminal(&birdd, &run, &cookie, Some(&own))
        .await
        .expect("the terminal opens");
    let (output, ended) = session(&mut socket, b"echo fresh; exit 0\n").await;
    assert!(output.contains("fresh"), "{output:?}");
    assert_eq!(ended, Frame::Exit(0));

    // closing the tab hangs the shell up instead of leaving it running in the machine
    let mut socket = open_terminal(&birdd, &exec, &cookie, Some(&own))
        .await
        .expect("the terminal opens");
    let typed = Frame::Data(b"sleep 321\n".to_vec()).encode();
    socket.send(Message::Binary(typed.into())).await.unwrap();
    let ps = ["ps", "-o", "args"];
    let deadline = tokio::time::Instant::now() + TERMINAL_WAIT;
    while !birdd
        .exec(&birdd.root, &production, "web", &ps)
        .await
        .1
        .contains("sleep 321")
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the sleep never started"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    drop(socket);
    while birdd
        .exec(&birdd.root, &production, "web", &ps)
        .await
        .1
        .contains("sleep 321")
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "the shell outlived its tab"
        );
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    // a page elsewhere cannot open one with the cookie
    let refused = open_terminal(&birdd, &exec, &cookie, None).await;
    let Err(err) = refused else {
        panic!("a terminal opened without an origin");
    };
    let tungstenite::Error::Http(response) = *err else {
        panic!("not an http refusal: {err}");
    };
    assert_eq!(response.status(), StatusCode::FORBIDDEN);

    let missing = format!("{production}/services/nope/exec/tty?{shell}");
    let Err(err) = open_terminal(&birdd, &missing, &cookie, Some(&own)).await else {
        panic!("a terminal opened for a missing service");
    };
    let tungstenite::Error::Http(response) = *err else {
        panic!("not an http refusal: {err}");
    };
    assert_eq!(response.status(), StatusCode::NOT_FOUND);

    let service = format!("{production}/services/web");
    assert_eq!(
        birdd.delete(&birdd.root, &service).await.status,
        StatusCode::NO_CONTENT
    );
    birdd.stop().await;
}
