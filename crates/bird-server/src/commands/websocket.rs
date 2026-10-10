use std::future::Future;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use bird_api::Frame;
use bird_core::Name;
use futures_util::{SinkExt, StreamExt};
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
use tokio::io::{AsyncReadExt, AsyncWriteExt, DuplexStream};
use tokio::sync::OwnedSemaphorePermit;

use super::tty::{self, Remote, Stdio};
use crate::Result;
use crate::state::AppState;

const PIPE_BYTES: usize = 64 * 1024;
// how long a client that was told the session ended has to close its side
const CLOSE_WAIT: Duration = Duration::from_secs(5);

// a browser's terminal: bird-tty frames ride in binary messages, which may split or join them.
// The command starts only once the socket is open, so a failed upgrade leaves nothing running.
pub(crate) async fn serve(
    state: AppState,
    name: Name,
    mut socket: WebSocket,
    start: impl Future<Output = Result<(TokioIo<Upgraded>, Remote)>>,
    stdio: Stdio,
    permit: OwnedSemaphorePermit,
) {
    let (io, remote) = match start.await {
        Ok(started) => started,
        Err(err) => {
            let refusal = Frame::Error(err.to_string()).encode();
            if let Err(err) = socket.send(Message::Binary(refusal.into())).await {
                tracing::debug!(service = %name, error = %err, "could not tell the client why");
            }
            return;
        }
    };
    let (ours, theirs) = tokio::io::duplex(PIPE_BYTES);
    tokio::join!(
        tty::session(&state, &name, ours, io, remote, stdio),
        pump(socket, theirs),
    );
    drop(permit);
}

// carries bytes between the socket and the session until both are done with each other
async fn pump(socket: WebSocket, stream: DuplexStream) {
    let (mut sink, mut source) = socket.split();
    let (mut from_session, mut to_session) = tokio::io::split(stream);
    let inbound = async move {
        while let Some(Ok(message)) = source.next().await {
            match message {
                Message::Binary(bytes) => {
                    if to_session.write_all(&bytes).await.is_err() {
                        break;
                    }
                }
                Message::Close(_) => break,
                Message::Text(_) | Message::Ping(_) | Message::Pong(_) => {}
            }
        }
        // the session reads end of file and hangs up, as when a bird-tty client leaves
        if let Err(err) = to_session.shutdown().await {
            tracing::debug!(error = %err, "terminal session already gone");
        }
    };
    let outbound = async move {
        let mut chunk = vec![0_u8; PIPE_BYTES];
        loop {
            let read = from_session.read(&mut chunk).await.unwrap_or(0);
            let Some(bytes) = chunk.get(..read).filter(|bytes| !bytes.is_empty()) else {
                break;
            };
            if sink
                .send(Message::Binary(bytes.to_vec().into()))
                .await
                .is_err()
            {
                return;
            }
        }
        if let Err(err) = sink.send(Message::Close(None)).await {
            tracing::debug!(error = %err, "terminal client left before the close");
        }
    };
    tokio::pin!(inbound, outbound);
    tokio::select! {
        () = &mut inbound => outbound.await,
        () = &mut outbound => {
            if tokio::time::timeout(CLOSE_WAIT, inbound).await.is_err() {
                tracing::debug!("terminal client never closed its side");
            }
        }
    }
}
