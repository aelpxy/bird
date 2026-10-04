use std::io::Read;
use std::time::Duration;

use anyhow::{Result, bail};
use bird_api::{Frame, TTY_UPGRADE};
use bird_core::Name;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::signal::unix::{SignalKind, signal};
use tokio::sync::mpsc;

use super::build::encode;
use super::exec::RemoteExit;
use crate::client::ApiClient;
use crate::ui::terminal::{self, RawMode};

const TIMEOUT: Duration = Duration::from_secs(30);
pub(super) const CHUNK_BYTES: usize = 32 * 1024;
const INPUT_BUFFER: usize = 64;
// bash when the image has it, for history and line editing, otherwise whatever sh is
const DEFAULT_SHELL: [&str; 3] = [
    "/bin/sh",
    "-c",
    "if command -v bash >/dev/null 2>&1; then exec bash; else exec sh; fi",
];

// where the terminal runs: in a running machine, or in a fresh container from the service's image
pub(crate) enum Place<'a> {
    Machine(Option<&'a str>),
    // the command goes to the image's entrypoint unless it skips it
    NewContainer { skip_entrypoint: bool },
}

pub(crate) async fn session(
    client: &ApiClient,
    name: &Name,
    place: Place<'_>,
    command: Vec<String>,
) -> Result<()> {
    let command = if command.is_empty() {
        DEFAULT_SHELL.map(String::from).to_vec()
    } else {
        command
    };
    let (cols, rows) = terminal::size();
    let path = format!(
        "{}&cols={cols}&rows={rows}",
        path(client, name, &place, "tty", &command)?
    );
    let io = client.upgrade(&path, TTY_UPGRADE, TIMEOUT).await?;
    let raw = RawMode::enable()?;
    let outcome = relay(io).await;
    drop(raw);
    match outcome? {
        0 => Ok(()),
        code => Err(RemoteExit(code).into()),
    }
}

// `mode` is `tty` or `pipe`
pub(super) fn path(
    client: &ApiClient,
    name: &Name,
    place: &Place<'_>,
    mode: &str,
    command: &[String],
) -> Result<String> {
    let endpoint = match place {
        Place::Machine(_) => "exec",
        Place::NewContainer { .. } => "run",
    };
    let mut path = client.scoped(&format!(
        "services/{name}/{endpoint}/{mode}?command={}",
        encode(&serde_json::to_string(command)?)
    ));
    match place {
        Place::Machine(Some(machine)) => {
            path.push_str("&machine=");
            path.push_str(&encode(machine));
        }
        Place::NewContainer {
            skip_entrypoint: true,
        } => path.push_str("&skip_entrypoint=true"),
        Place::Machine(None)
        | Place::NewContainer {
            skip_entrypoint: false,
        } => {}
    }
    Ok(path)
}

async fn relay(io: TokioIo<Upgraded>) -> Result<i32> {
    let (mut from_birdd, mut to_birdd) = tokio::io::split(io);
    let mut keys = read_input();
    let mut resized = signal(SignalKind::window_change())?;
    let mut stdout = tokio::io::stdout();
    let mut buffer = Vec::new();
    let mut chunk = vec![0_u8; CHUNK_BYTES];
    let mut typing = true;
    loop {
        tokio::select! {
            typed = keys.recv(), if typing => match typed {
                Some(bytes) => to_birdd.write_all(&Frame::Data(bytes?).encode()).await?,
                None => typing = false,
            },
            _ = resized.recv() => {
                let (cols, rows) = terminal::size();
                to_birdd.write_all(&Frame::Resize { cols, rows }.encode()).await?;
            }
            read = from_birdd.read(&mut chunk) => {
                let Some(bytes) = chunk.get(..read?).filter(|bytes| !bytes.is_empty()) else {
                    bail!("the connection to birdd closed before the command finished");
                };
                buffer.extend_from_slice(bytes);
                while let Some(frame) = Frame::decode(&mut buffer)? {
                    match frame {
                        Frame::Data(output) => {
                            stdout.write_all(&output).await?;
                            stdout.flush().await?;
                        }
                        Frame::Exit(code) => return Ok(code),
                        Frame::Error(message) => bail!(message),
                        Frame::Resize { .. } | Frame::Stderr(_) | Frame::Eof => {}
                    }
                }
            }
        }
    }
}

// a plain thread: tokio's stdin would hold up the exit until one more key was pressed
pub(super) fn read_input() -> mpsc::Receiver<std::io::Result<Vec<u8>>> {
    let (input, receiver) = mpsc::channel(INPUT_BUFFER);
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut chunk = vec![0_u8; CHUNK_BYTES];
        loop {
            let read = match stdin.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => Ok(chunk.get(..read).unwrap_or_default().to_vec()),
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => Err(err),
            };
            let failed = read.is_err();
            if input.blocking_send(read).is_err() || failed {
                break;
            }
        }
    });
    receiver
}
