use std::time::Duration;

use bird_api::Frame;
use bird_core::Name;
use bird_podman::TtySession;
use hyper::upgrade::OnUpgrade;
use hyper_util::rt::TokioIo;
use rustix::process::{Pid, Signal, kill_process};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::state::AppState;

const CHUNK_BYTES: usize = 32 * 1024;
const EXIT_POLL: Duration = Duration::from_millis(100);
const EXIT_WAIT: Duration = Duration::from_secs(2);
const HANG_UP_GRACE: Duration = Duration::from_secs(2);
// each terminal holds two connections and two tasks; this bounds what one token holder can open
pub(crate) const MAX_TERMINALS: usize = 64;
const SHUTDOWN_WAIT: Duration = Duration::from_secs(10);

enum Ended {
    Exited(i32),
    Left,
}

// no idle timeout: an open shell waiting for input is the normal state, shutdown still ends it
pub(crate) async fn serve(
    state: AppState,
    name: Name,
    upgrade: OnUpgrade,
    session: TtySession,
    permit: OwnedSemaphorePermit,
) {
    let id = session.id().to_owned();
    let client = match upgrade.await {
        Ok(upgraded) => TokioIo::new(upgraded),
        Err(err) => {
            tracing::debug!(service = %name, error = %err, "terminal client never connected");
            hang_up(&state, &id).await;
            return;
        }
    };
    match bridge(&state, client, session).await {
        Ended::Exited(code) => {
            tracing::info!(service = %name, code, "terminal closed");
        }
        Ended::Left => {
            tracing::info!(service = %name, "terminal client left, hanging up");
            hang_up(&state, &id).await;
        }
    }
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
    session: TtySession,
) -> Ended {
    let id = session.id().to_owned();
    let (mut client_read, mut client_write) = tokio::io::split(client);
    let (mut process_read, mut process_write) = tokio::io::split(session.io);
    let input = async {
        let mut buffer = Vec::new();
        let mut chunk = vec![0_u8; CHUNK_BYTES];
        loop {
            let read = client_read.read(&mut chunk).await?;
            let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
                return Ok::<_, Box<dyn std::error::Error + Send + Sync>>(());
            };
            buffer.extend_from_slice(bytes);
            while let Some(frame) = Frame::decode(&mut buffer)? {
                match frame {
                    Frame::Data(bytes) => process_write.write_all(&bytes).await?,
                    Frame::Resize { cols, rows } => {
                        if let Err(err) = state.podman.resize_exec(&id, cols, rows).await {
                            tracing::debug!(error = %err, "could not resize terminal");
                        }
                    }
                    Frame::Exit(_) | Frame::Error(_) => {}
                }
            }
        }
    };
    let output = async {
        let mut chunk = vec![0_u8; CHUNK_BYTES];
        loop {
            let read = process_read.read(&mut chunk).await?;
            let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
                return Ok::<_, std::io::Error>(());
            };
            client_write
                .write_all(&Frame::Data(bytes.to_vec()).encode())
                .await?;
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
    let code = exit_code(state, &id).await;
    let _ = client_write.write_all(&Frame::Exit(code).encode()).await;
    let _ = client_write.shutdown().await;
    Ended::Exited(code)
}

// the output ends a moment before podman records the exit
async fn exit_code(state: &AppState, id: &str) -> i32 {
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
        if let Err(err) = kill_process(pid, signal) {
            tracing::warn!(exec = id, error = %err, "could not hang up terminal process");
            return;
        }
        tokio::time::sleep(HANG_UP_GRACE).await;
    }
}
