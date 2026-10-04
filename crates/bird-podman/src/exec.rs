use bytes::Bytes;
use http_body_util::Full;
use serde::{Deserialize, Serialize};

use crate::client::DEFAULT_TIMEOUT;
use crate::error::check;
use crate::follow::LogFollower;
use crate::query::encode;
use crate::{Error, Podman, Result};

// a command running inside a container: read its output, then ask for its exit code
pub struct ExecSession {
    id: String,
    pub output: LogFollower,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct CreateExec<'a> {
    cmd: &'a [String],
    attach_stdout: bool,
    attach_stderr: bool,
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
    exit_code: i32,
}

impl Podman {
    pub async fn exec(&self, container: &str, command: &[String]) -> Result<ExecSession> {
        let create = CreateExec {
            cmd: command,
            attach_stdout: true,
            attach_stderr: true,
        };
        let path = format!("/containers/{}/exec", encode(container));
        let response = self.post_json(&path, &create).await?;
        let body = check(response, || format!("container {container}"))?;
        let id = serde_json::from_slice::<Created>(&body)?.id;

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
            output: LogFollower::new(streamed.body),
        })
    }

    // only meaningful once the output has ended, before that podman reports the command as running
    pub async fn exec_exit_code(&self, session: &ExecSession) -> Result<i32> {
        let response = self
            .get(&format!("/exec/{}/json", encode(&session.id)))
            .await?;
        let body = check(response, || format!("exec {}", session.id))?;
        Ok(serde_json::from_slice::<ExecState>(&body)?.exit_code)
    }
}
