use std::io::IsTerminal;
use std::time::Duration;

use anyhow::{Context, Result, bail};
use bird_api::{Frame, TTY_UPGRADE};
use bird_core::Name;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use super::exec::RemoteExit;
use super::tty::{CHUNK_BYTES, Place, path, read_input};
use crate::client::ApiClient;

const TIMEOUT: Duration = Duration::from_secs(30);

// stdin, stdout and stderr pass through as bytes, so dumps and restores work through pipes
pub(crate) async fn session(
    client: &ApiClient,
    name: &Name,
    place: Place<'_>,
    command: Vec<String>,
) -> Result<()> {
    if command.is_empty() {
        bail!("give a command to run; an interactive shell needs a terminal on stdin and stdout");
    }
    let path = path(name, &place, "pipe", &command)?;
    let io = client.upgrade(&path, TTY_UPGRADE, TIMEOUT).await?;
    match relay(io).await? {
        0 => Ok(()),
        code => Err(RemoteExit(code).into()),
    }
}

async fn relay(io: TokioIo<Upgraded>) -> Result<i32> {
    let (mut from_birdd, mut to_birdd) = tokio::io::split(io);
    // a terminal is not read, like `docker exec` without -i, so the command gets end of file
    let mut input = if std::io::stdin().is_terminal() {
        mpsc::channel(1).1
    } else {
        read_input()
    };
    let mut reading = true;
    let mut stdout = tokio::io::stdout();
    let mut stderr = tokio::io::stderr();
    let mut buffer = Vec::new();
    let mut chunk = vec![0_u8; CHUNK_BYTES];
    loop {
        tokio::select! {
            read = input.recv(), if reading => {
                let frame = if let Some(bytes) = read {
                    Frame::Data(bytes.context("could not read stdin")?)
                } else {
                    reading = false;
                    Frame::Eof
                };
                to_birdd.write_all(&frame.encode()).await?;
            }
            read = from_birdd.read(&mut chunk) => {
                let Some(bytes) = chunk.get(..read?).filter(|bytes| !bytes.is_empty()) else {
                    bail!("the connection to birdd closed before the command finished");
                };
                buffer.extend_from_slice(bytes);
                while let Some(frame) = Frame::decode(&mut buffer)? {
                    match frame {
                        Frame::Data(output) => write_now(&mut stdout, &output).await?,
                        Frame::Stderr(output) => write_now(&mut stderr, &output).await?,
                        Frame::Exit(code) => return Ok(code),
                        Frame::Error(message) => bail!(message),
                        Frame::Resize { .. } | Frame::Eof => {}
                    }
                }
            }
        }
    }
}

// flushed per chunk, so prompts and progress without a newline show up as they are written
async fn write_now(to: &mut (impl AsyncWrite + Unpin), bytes: &[u8]) -> std::io::Result<()> {
    to.write_all(bytes).await?;
    to.flush().await
}
