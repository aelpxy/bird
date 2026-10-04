use std::fmt;
use std::io::Write;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use bird_api::{CommandEvent, ExecRequest, LogStream, RunRequest};
use bird_core::{Command, Name};
use serde::Serialize;

use crate::client::ApiClient;
use crate::ui::Output;

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
    out: Output,
) -> Result<()> {
    if command.is_empty() {
        bail!("give a command to run; an interactive shell needs a terminal on stdin and stdout");
    }
    let request = ExecRequest {
        command: Command::try_from(command)?,
        machine,
    };
    stream(client, &format!("/v1/services/{name}/exec"), &request, out).await
}

pub(crate) async fn run(
    client: &ApiClient,
    name: &Name,
    command: Vec<String>,
    out: Output,
) -> Result<()> {
    if command.is_empty() {
        bail!("give a command to run; an interactive shell needs a terminal on stdin and stdout");
    }
    let request = RunRequest {
        command: Command::try_from(command)?,
    };
    stream(client, &format!("/v1/services/{name}/run"), &request, out).await
}

// flushed per chunk, so prompts and progress without a newline show up as they are written
fn write_now(mut to: impl Write, text: &str) -> std::io::Result<()> {
    to.write_all(text.as_bytes())?;
    to.flush()
}

async fn stream(
    client: &ApiClient,
    path: &str,
    request: &impl Serialize,
    out: Output,
) -> Result<()> {
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
                if out.json {
                    println!("{line}");
                }
                match event {
                    CommandEvent::Output { stream, text } if !out.json => match stream {
                        LogStream::Stdout => write_now(std::io::stdout().lock(), &text)?,
                        LogStream::Stderr => write_now(std::io::stderr().lock(), &text)?,
                    },
                    CommandEvent::Output { .. } => {}
                    finished => last = Some(finished),
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
