use std::fmt;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use bird_api::{CommandEvent, ExecRequest, RunRequest};
use bird_core::{Command, Name};
use serde::Serialize;

use crate::client::ApiClient;

// covers the response head; the command then runs until it exits or birdd's own limit
const TIMEOUT: Duration = Duration::from_secs(30);

// the remote command's exit code becomes bird's, so scripts can check it
#[derive(Debug)]
pub(crate) struct RemoteExit(pub(crate) i32);

impl fmt::Display for RemoteExit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "the command exited with code {}", self.0)
    }
}

impl std::error::Error for RemoteExit {}

pub(crate) async fn exec(
    client: &ApiClient,
    name: &Name,
    machine: Option<String>,
    command: Vec<String>,
) -> Result<()> {
    if command.is_empty() {
        bail!("give a command to run; an interactive shell needs a terminal on stdin and stdout");
    }
    let request = ExecRequest {
        command: Command::try_from(command)?,
        machine,
    };
    stream(
        client,
        &client.scoped(&format!("services/{name}/exec")),
        &request,
    )
    .await
}

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    command: Vec<String>,
    skip_entrypoint: bool,
) -> Result<()> {
    if command.is_empty() {
        bail!("give a command to run; an interactive shell needs a terminal on stdin and stdout");
    }
    let request = RunRequest {
        command: Command::try_from(command)?,
        skip_entrypoint,
    };
    stream(
        client,
        &client.scoped(&format!("services/{name}/run")),
        &request,
    )
    .await
}

// `--json`: each event is printed as birdd sent it
async fn stream(client: &ApiClient, path: &str, request: &impl Serialize) -> Result<()> {
    let mut last = None;
    client
        .upload_lines(
            path,
            "application/json",
            serde_json::to_vec(request)?,
            TIMEOUT,
            |line| {
                let event: CommandEvent =
                    serde_json::from_str(line).context("birdd sent an unexpected event")?;
                println!("{line}");
                if !matches!(event, CommandEvent::Output { .. }) {
                    last = Some(event);
                }
                Ok(())
            },
        )
        .await?;
    match last {
        Some(CommandEvent::Exited { code: 0 }) => Ok(()),
        Some(CommandEvent::Exited { code }) => Err(RemoteExit(code).into()),
        Some(CommandEvent::Failed { error }) => bail!(error),
        Some(CommandEvent::Output { .. }) | None => {
            bail!("the connection to birdd ended before the command finished")
        }
    }
}
