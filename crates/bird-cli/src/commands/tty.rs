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
const CHUNK_BYTES: usize = 32 * 1024;
const KEY_BUFFER: usize = 64;
// bash when the image has it, for history and line editing, otherwise whatever sh is
const DEFAULT_SHELL: [&str; 3] = [
    "/bin/sh",
    "-c",
    "if command -v bash >/dev/null 2>&1; then exec bash; else exec sh; fi",
];

// where the terminal runs: in a running machine, or in a fresh container from the service's image
pub(crate) enum Place<'a> {
    Machine(Option<&'a str>),
    NewContainer,
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
    let endpoint = match place {
        Place::Machine(_) => "exec",
        Place::NewContainer => "run",
    };
    let mut path = format!(
        "/v1/services/{name}/{endpoint}/tty?command={}&cols={cols}&rows={rows}",
        encode(&serde_json::to_string(&command)?)
    );
    if let Place::Machine(Some(machine)) = place {
        path.push_str("&machine=");
        path.push_str(&encode(machine));
    }
    let io = client.upgrade(&path, TTY_UPGRADE, TIMEOUT).await?;
    let raw = RawMode::enable()?;
    let outcome = relay(io).await;
    drop(raw);
    match outcome? {
        0 => Ok(()),
        code => Err(RemoteExit(code).into()),
    }
}

async fn relay(io: TokioIo<Upgraded>) -> Result<i32> {
    let (mut from_birdd, mut to_birdd) = tokio::io::split(io);
    let mut keys = read_keys();
    let mut resized = signal(SignalKind::window_change())?;
    let mut stdout = tokio::io::stdout();
    let mut buffer = Vec::new();
    let mut chunk = vec![0_u8; CHUNK_BYTES];
    let mut typing = true;
    loop {
        tokio::select! {
            typed = keys.recv(), if typing => match typed {
                Some(bytes) => to_birdd.write_all(&Frame::Data(bytes).encode()).await?,
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
                        Frame::Resize { .. } => {}
                    }
                }
            }
        }
    }
}

// a plain thread: tokio's stdin would hold up the exit until one more key was pressed
fn read_keys() -> mpsc::Receiver<Vec<u8>> {
    let (keys, receiver) = mpsc::channel(KEY_BUFFER);
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin().lock();
        let mut chunk = [0_u8; 4096];
        loop {
            let typed = match stdin.read(&mut chunk) {
                Ok(0) | Err(_) => break,
                Ok(read) => chunk.get(..read).unwrap_or_default().to_vec(),
            };
            if keys.blocking_send(typed).is_err() {
                break;
            }
        }
    });
    receiver
}
