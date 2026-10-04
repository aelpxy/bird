// an in-process birdd against the real podman, with its own data dir, ports and networks
#![allow(
    dead_code,
    reason = "each test binary uses a different part of the harness"
)]

use std::net::TcpListener;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use bird_server::{Config, run_until};
use clap::Parser;
use reqwest::{Method, StatusCode};
use serde_json::{Value, json};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

pub const IMAGE: &str = "docker.io/traefik/whoami:latest";
pub const SHELL_IMAGE: &str = "docker.io/library/nginx:alpine";
const STARTUP: Duration = Duration::from_secs(30);
const REQUEST_TIMEOUT: Duration = Duration::from_mins(3);

static NEXT: AtomicU32 = AtomicU32::new(0);

pub struct Birdd {
    pub api: String,
    pub root: String,
    // starts every project name, so networks never clash with another birdd on this podman
    pub prefix: String,
    network: String,
    data_dir: PathBuf,
    args: Vec<String>,
    stop: Option<oneshot::Sender<()>>,
    task: Option<JoinHandle<()>>,
    http: reqwest::Client,
}

pub struct Response {
    pub status: StatusCode,
    pub body: Value,
}

impl Birdd {
    pub async fn start() -> Self {
        let id = format!(
            "t{}x{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let data_dir = std::env::temp_dir().join(format!("bird-it-{id}"));
        let network = format!("bird-it-{id}");
        let api = format!("127.0.0.1:{}", free_port());
        let args = vec![
            "birdd".to_owned(),
            "--data-dir".to_owned(),
            data_dir.to_str().expect("temp paths are utf-8").to_owned(),
            "--api-addr".to_owned(),
            api.clone(),
            "--proxy-addr".to_owned(),
            format!("127.0.0.1:{}", free_port()),
            "--network".to_owned(),
            network.clone(),
            "--no-db-backup".to_owned(),
        ];
        // reqwest is built without a crypto provider of its own; an error means one is installed
        let _ = rustls::crypto::ring::default_provider().install_default();
        let http = reqwest::Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .build()
            .expect("an http client");
        let (stop, task) = spawn(&args);
        wait_until_up(&http, &api).await;
        let root = std::fs::read_to_string(data_dir.join("api-token"))
            .expect("birdd wrote its token")
            .trim()
            .to_owned();
        Self {
            api,
            root,
            prefix: id,
            network,
            data_dir,
            args,
            stop: Some(stop),
            task: Some(task),
            http,
        }
    }

    // stops birdd and starts it again on the same data, ports and networks
    pub async fn restart(&mut self) {
        self.shut_down().await;
        let (stop, task) = spawn(&self.args);
        self.stop = Some(stop);
        self.task = Some(task);
        wait_until_up(&self.http, &self.api).await;
    }

    // a unique name for a project, so its networks are this test's own
    pub fn project(&self, name: &str) -> String {
        format!("{}{name}", self.prefix)
    }

    pub async fn call(
        &self,
        token: &str,
        method: Method,
        path: &str,
        body: Option<Value>,
    ) -> Response {
        let mut request = self
            .http
            .request(method, format!("http://{}{path}", self.api))
            .bearer_auth(token);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().await.expect("birdd answers");
        let status = response.status();
        let text = response.text().await.expect("a readable body");
        let body = if text.is_empty() {
            Value::Null
        } else {
            serde_json::from_str(&text).unwrap_or(Value::String(text))
        };
        Response { status, body }
    }

    pub async fn get(&self, token: &str, path: &str) -> Response {
        self.call(token, Method::GET, path, None).await
    }

    pub async fn post(&self, token: &str, path: &str, body: Value) -> Response {
        self.call(token, Method::POST, path, Some(body)).await
    }

    pub async fn put(&self, token: &str, path: &str, body: Value) -> Response {
        self.call(token, Method::PUT, path, Some(body)).await
    }

    pub async fn delete(&self, token: &str, path: &str) -> Response {
        self.call(token, Method::DELETE, path, None).await
    }

    // a user with that role and their first token
    pub async fn user(&self, name: &str, role: &str) -> String {
        let created = self
            .post(
                &self.root,
                "/v1/users",
                json!({ "name": name, "role": role }),
            )
            .await;
        assert_eq!(created.status, StatusCode::CREATED, "{:?}", created.body);
        created
            .body
            .pointer("/token/token")
            .and_then(Value::as_str)
            .expect("a new user comes with a token")
            .to_owned()
    }

    pub async fn deploy(&self, token: &str, scope: &str, name: &str, image: &str) -> Response {
        self.post(
            token,
            &format!("{scope}/deploy"),
            json!({ "name": name, "image": image, "port": 80 }),
        )
        .await
    }

    // runs a command in a service's machine, returning its exit code and output; a command that
    // could not run gives -1 and the reason
    pub async fn exec(
        &self,
        token: &str,
        scope: &str,
        service: &str,
        command: &[&str],
    ) -> (i32, String) {
        let response = self
            .http
            .post(format!(
                "http://{}{scope}/services/{service}/exec",
                self.api
            ))
            .bearer_auth(token)
            .json(&json!({ "command": command }))
            .send()
            .await
            .expect("birdd answers");
        assert!(
            response.status().is_success(),
            "exec refused: {}",
            response.status()
        );
        let text = response.text().await.expect("a readable stream");
        let mut output = String::new();
        for line in text.lines() {
            let event: Value = serde_json::from_str(line).expect("birdd streams json events");
            let field = |name: &str| event.get(name).cloned().unwrap_or(Value::Null);
            match field("type").as_str() {
                Some("output") => output.push_str(field("text").as_str().unwrap_or_default()),
                Some("exited") => {
                    let code = field("code")
                        .as_i64()
                        .and_then(|code| i32::try_from(code).ok());
                    return (code.unwrap_or(-1), output);
                }
                _ => return (-1, format!("{output}{event}")),
            }
        }
        (
            -1,
            format!("{output}(the stream ended without an exit code)"),
        )
    }

    pub async fn stop(mut self) {
        self.shut_down().await;
    }

    async fn shut_down(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            let _ = tokio::time::timeout(Duration::from_secs(30), task).await;
        }
    }
}

// scope paths like `/v1/projects/p/environments/e`
pub fn scope(project: &str, environment: &str) -> String {
    format!("/v1/projects/{project}/environments/{environment}")
}

impl Drop for Birdd {
    // whatever a test leaves, also when it fails halfway: its containers, networks and data
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        let mut networks = podman(&["network", "ls", "--format", "{{.Name}}"])
            .lines()
            .filter(|name| name.starts_with(&format!("bird_{}", self.prefix)))
            .map(str::to_owned)
            .collect::<Vec<_>>();
        networks.push(self.network.clone());
        for network in &networks {
            let containers = podman(&["ps", "-aq", "--filter", &format!("network={network}")]);
            for container in containers.lines() {
                podman(&["rm", "-f", "-t", "0", container]);
            }
            podman(&["network", "rm", "-f", network]);
        }
        let _ = std::fs::remove_dir_all(&self.data_dir);
    }
}

fn spawn(args: &[String]) -> (oneshot::Sender<()>, JoinHandle<()>) {
    let config = Config::try_parse_from(args).expect("valid test config");
    let (stop, stopped) = oneshot::channel();
    let task = tokio::spawn(async move {
        let stopped = async move {
            let _ = stopped.await;
        };
        if let Err(err) = Box::pin(run_until(config, stopped)).await {
            eprintln!("birdd stopped with an error: {err}");
        }
    });
    (stop, task)
}

async fn wait_until_up(http: &reqwest::Client, api: &str) {
    let deadline = tokio::time::Instant::now() + STARTUP;
    while http
        .get(format!("http://{api}/v1/openapi.json"))
        .send()
        .await
        .is_err()
    {
        assert!(
            tokio::time::Instant::now() < deadline,
            "birdd never started"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn podman(args: &[&str]) -> String {
    let output = Command::new("podman")
        .args(args)
        .output()
        .expect("podman is installed");
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .and_then(|listener| listener.local_addr())
        .expect("a free port")
        .port()
}
