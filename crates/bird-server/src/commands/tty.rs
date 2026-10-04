use std::time::Duration;

use bird_api::Frame;
use bird_core::Name;
use bird_podman::{Demux, LogStream};
use hyper::upgrade::{OnUpgrade, Upgraded};
use hyper_util::rt::TokioIo;
use rustix::process::{Pid, Signal, kill_process, kill_process_group};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::state::AppState;

const CHUNK_BYTES: usize = 32 * 1024;
const EXIT_POLL: Duration = Duration::from_millis(100);
const EXIT_WAIT: Duration = Duration::from_secs(2);
const HANG_UP_GRACE: Duration = Duration::from_secs(2);
const LINGER: Duration = Duration::from_secs(5);
// each terminal holds two connections and two tasks; this bounds what one token holder can open
pub(crate) const MAX_TERMINALS: usize = 64;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(10);

// how the command's input and output are carried
#[derive(Debug, Clone, Copy)]
pub(crate) enum Stdio {
    // one raw stream both ways, sized like the client's terminal
    Terminal { cols: u16, rows: u16 },
    // stdin, stdout and stderr kept apart and passed as bytes, for dumps and restores
    Piped,
}

// what the terminal is connected to, which decides how it is resized, read and ended
pub(crate) enum Remote {
    // a command in a running machine; podman keeps it running when its client leaves
    Exec(String),
    // a one-off container whose main process is the command; removing it ends everything
    Container(String),
}

impl Remote {
    async fn resize(&self, state: &AppState, cols: u16, rows: u16) {
        let resized = match self {
            Self::Exec(id) => state.podman.resize_exec(id, cols, rows).await,
            Self::Container(id) => state.podman.resize_container(id, cols, rows).await,
        };
        if let Err(err) = resized {
            tracing::debug!(error = %err, "could not resize terminal");
        }
    }

    // the output ends a moment before podman records the exit
    async fn exit_code(&self, state: &AppState) -> i32 {
        match self {
            Self::Exec(id) => exec_exit_code(state, id).await,
            Self::Container(id) => state
                .podman
                .wait_container(id, EXIT_WAIT)
                .await
                .unwrap_or_else(|err| {
                    tracing::debug!(error = %err, "could not read terminal exit code");
                    -1
                }),
        }
    }

    // runs however the session ended, so nothing is left behind in the machine or on the host;
    // hanging up an exec that already exited does nothing
    async fn end(&self, state: &AppState) {
        match self {
            Self::Exec(id) => hang_up(state, id).await,
            Self::Container(id) => {
                if let Err(err) = state.podman.remove_container(id).await {
                    tracing::warn!(container = id, error = %err, "could not remove one-off container");
                }
            }
        }
    }
}

enum Ended {
    Exited(i32),
    Left,
}

// no idle timeout: an open shell waiting for input is the normal state, shutdown still ends it
pub(crate) async fn serve(
    state: AppState,
    name: Name,
    upgrade: OnUpgrade,
    io: TokioIo<Upgraded>,
    remote: Remote,
    stdio: Stdio,
    permit: OwnedSemaphorePermit,
) {
    let client = match upgrade.await {
        Ok(upgraded) => TokioIo::new(upgraded),
        Err(err) => {
            tracing::debug!(service = %name, error = %err, "terminal client never connected");
            remote.end(&state).await;
            return;
        }
    };
    match bridge(&state, client, io, &remote, stdio).await {
        Ended::Exited(code) => tracing::info!(service = %name, code, "terminal closed"),
        Ended::Left => tracing::info!(service = %name, "terminal client left, hanging up"),
    }
    remote.end(&state).await;
    drop(permit);
}

// birdd exits once every terminal released its permit, so none is left running in a machine
pub(crate) async fn wait_for_hang_ups(terminals: &Semaphore) {
    let all = u32::try_from(MAX_TERMINALS).unwrap_or(u32::MAX);
    if tokio::time::timeout(SHUTDOWN_WAIT, terminals.acquire_many(all))
        .await
        .is_err()
    {
        tracing::warn!("some terminals were still closing when birdd stopped");
    }
}

async fn bridge(
    state: &AppState,
    client: impl AsyncRead + AsyncWrite,
    process: TokioIo<Upgraded>,
    remote: &Remote,
    stdio: Stdio,
) -> Ended {
    let (mut client_read, mut client_write) = tokio::io::split(client);
    let (mut process_read, mut process_write) = tokio::io::split(process);
    let input = async {
        let mut buffer = Vec::new();
        let mut chunk = vec![0_u8; CHUNK_BYTES];
        let mut stdin_open = true;
        loop {
            let read = client_read.read(&mut chunk).await?;
            let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
                return Ok::<_, Box<dyn std::error::Error + Send + Sync>>(());
            };
            buffer.extend_from_slice(bytes);
            while let Some(frame) = Frame::decode(&mut buffer)? {
                match frame {
                    // a command may stop reading early; its output, not its input, ends the session
                    Frame::Data(bytes) if stdin_open => {
                        if let Err(err) = process_write.write_all(&bytes).await {
                            tracing::debug!(error = %err, "command stopped taking input");
                            stdin_open = false;
                        }
                    }
                    Frame::Eof if stdin_open => {
                        stdin_open = false;
                        if let Err(err) = process_write.shutdown().await {
                            tracing::debug!(error = %err, "could not close the command's input");
                        }
                    }
                    Frame::Resize { cols, rows } => remote.resize(state, cols, rows).await,
                    Frame::Data(_)
                    | Frame::Eof
                    | Frame::Exit(_)
                    | Frame::Error(_)
                    | Frame::Stderr(_) => {}
                }
            }
        }
    };
    let output = async {
        let mut demux = Demux::default();
        let mut chunk = vec![0_u8; CHUNK_BYTES];
        loop {
            // podman resets the connection when the command exits with input still unread
            let read = process_read.read(&mut chunk).await.unwrap_or_else(|err| {
                tracing::debug!(error = %err, "command output ended");
                0
            });
            let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
                return Ok::<_, Box<dyn std::error::Error + Send + Sync>>(());
            };
            match stdio {
                Stdio::Terminal { .. } => {
                    client_write
                        .write_all(&Frame::Data(bytes.to_vec()).encode())
                        .await?;
                }
                Stdio::Piped => {
                    for (stream, piece) in demux.push(bytes)? {
                        let frame = match stream {
                            LogStream::Stdout => Frame::Data(piece.to_vec()),
                            LogStream::Stderr => Frame::Stderr(piece.to_vec()),
                        };
                        client_write.write_all(&frame.encode()).await?;
                    }
                }
            }
        }
    };
    let finished = tokio::select! {
        result = input => {
            if let Err(err) = result {
                tracing::debug!(error = %err, "terminal input ended");
            }
            return Ended::Left;
        }
        result = output => result,
        () = state.shutdown.wait() => {
            let goodbye = Frame::Error("birdd is shutting down".to_owned()).encode();
            let _ = client_write.write_all(&goodbye).await;
            return Ended::Left;
        }
    };
    if let Err(err) = finished {
        tracing::debug!(error = %err, "terminal output ended");
        return Ended::Left;
    }
    let code = remote.exit_code(state).await;
    let _ = client_write.write_all(&Frame::Exit(code).encode()).await;
    let _ = client_write.shutdown().await;
    // drain the client's remaining input, or closing resets the connection and loses the exit frame
    let _ = tokio::time::timeout(
        LINGER,
        tokio::io::copy(&mut client_read, &mut tokio::io::sink()),
    )
    .await;
    Ended::Exited(code)
}

async fn exec_exit_code(state: &AppState, id: &str) -> i32 {
    let deadline = tokio::time::Instant::now() + EXIT_WAIT;
    loop {
        match state.podman.inspect_exec(id).await {
            Ok(info) if !info.running => return info.exit_code,
            Ok(_) if tokio::time::Instant::now() < deadline => {}
            Ok(_) => return -1,
            Err(err) => {
                tracing::debug!(error = %err, "could not read terminal exit code");
                return -1;
            }
        }
        tokio::time::sleep(EXIT_POLL).await;
    }
}

// podman keeps a terminal process running after its client leaves, so it gets what a closed
// terminal sends; rootless podman runs it as birdd's own user, so birdd may signal it
async fn hang_up(state: &AppState, id: &str) {
    for signal in [Signal::HUP, Signal::KILL] {
        let Ok(info) = state.podman.inspect_exec(id).await else {
            return;
        };
        let pid = info
            .pid
            .and_then(|pid| i32::try_from(pid).ok())
            .and_then(Pid::from_raw);
        let (true, Some(pid)) = (info.running, pid) else {
            return;
        };
        // podman starts an exec as its own group leader; without a terminal nothing else passes the
        // hang-up on to the command's children
        let sent = kill_process_group(pid, signal).or_else(|_| kill_process(pid, signal));
        if let Err(err) = sent {
            tracing::warn!(exec = id, error = %err, "could not hang up terminal process");
            return;
        }
        tokio::time::sleep(HANG_UP_GRACE).await;
    }
}
