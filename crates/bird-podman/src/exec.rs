use bytes::Bytes;
use http_body_util::Full;
use hyper::Method;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use serde::{Deserialize, Serialize};

use crate::client::DEFAULT_TIMEOUT;
use crate::error::check;
use crate::output::OutputFollower;
use crate::query::encode;
use crate::{Error, Podman, Result};

// a command running inside a container: read its output, then ask for its exit code
pub struct ExecSession {
    id: String,
    pub output: OutputFollower,
}

// a command on a terminal: one raw stream both ways, with no stdout and stderr framing
pub struct TtySession {
    id: String,
    pub io: TokioIo<Upgraded>,
}

impl TtySession {
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecInfo {
    pub running: bool,
    pub exit_code: i32,
    // the process id on the host, signalable by the user rootless podman runs as
    pub pid: Option<u32>,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the body podman's exec create endpoint takes"
)]
struct CreateExec<'a> {
    cmd: &'a [String],
    attach_stdin: bool,
    attach_stdout: bool,
    attach_stderr: bool,
    tty: bool,
}

#[derive(Deserialize)]
struct Created {
    #[serde(rename = "Id")]
    id: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct StartExec {
    detach: bool,
    tty: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ExecState {
    running: bool,
    exit_code: i32,
    #[serde(default)]
    pid: u32,
}

impl Podman {
    pub async fn exec(&self, container: &str, command: &[String]) -> Result<ExecSession> {
        let id = self.create_exec(container, command, false).await?;

        let start = serde_json::to_vec(&StartExec {
            detach: false,
            tty: false,
        })?;
        let path = format!("/exec/{}/start", encode(&id));
        let streamed = self
            .upload(
                &path,
                "application/json",
                Full::new(Bytes::from(start)),
                DEFAULT_TIMEOUT,
            )
            .await?;
        let status = streamed.status;
        if !status.is_success() {
            check(streamed.collect().await?, || format!("exec in {container}"))?;
            return Err(Error::Api {
                status: status.as_u16(),
                message: "unexpected response to an exec".to_owned(),
            });
        }
        Ok(ExecSession {
            id,
            output: OutputFollower::new(streamed.body),
        })
    }

    // only meaningful once the output has ended, before that podman reports the command as running
    pub async fn exec_exit_code(&self, session: &ExecSession) -> Result<i32> {
        Ok(self.inspect_exec(&session.id).await?.exit_code)
    }

    pub async fn exec_tty(&self, container: &str, command: &[String]) -> Result<TtySession> {
        let id = self.create_exec(container, command, true).await?;
        let start = StartExec {
            detach: false,
            tty: true,
        };
        let io = self
            .upgrade(&format!("/exec/{}/start", encode(&id)), &start)
            .await?;
        Ok(TtySession { id, io })
    }

    // only works while the command runs
    pub async fn resize_exec(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let path = format!("/exec/{}/resize?h={rows}&w={cols}", encode(id));
        let response = self.send(Method::POST, &path, DEFAULT_TIMEOUT).await?;
        check(response, || format!("exec {id}"))?;
        Ok(())
    }

    pub async fn inspect_exec(&self, id: &str) -> Result<ExecInfo> {
        let response = self.get(&format!("/exec/{}/json", encode(id))).await?;
        let body = check(response, || format!("exec {id}"))?;
        let state: ExecState = serde_json::from_slice(&body)?;
        Ok(ExecInfo {
            running: state.running,
            exit_code: state.exit_code,
            pid: Some(state.pid).filter(|pid| *pid > 0),
        })
    }

    async fn create_exec(&self, container: &str, command: &[String], tty: bool) -> Result<String> {
        let create = CreateExec {
            cmd: command,
            attach_stdin: tty,
            attach_stdout: true,
            attach_stderr: true,
            tty,
        };
        let path = format!("/containers/{}/exec", encode(container));
        let response = self.post_json(&path, &create).await?;
        let body = check(response, || format!("container {container}"))?;
        Ok(serde_json::from_slice::<Created>(&body)?.id)
    }
}
