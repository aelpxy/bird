use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::GracefulShutdown;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::forward::Forwarder;
use crate::{ProxyConfig, Routes};

const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);

pub struct Proxy {
    forwarder: Arc<Forwarder>,
    config: ProxyConfig,
}

impl Proxy {
    #[must_use]
    pub fn new(routes: Routes, config: ProxyConfig) -> Self {
        Self {
            forwarder: Arc::new(Forwarder::new(routes, &config)),
            config,
        }
    }

    pub async fn serve(&self, listener: TcpListener, shutdown: impl Future<Output = ()>) {
        let graceful = GracefulShutdown::new();
        let permits = Arc::new(Semaphore::new(self.config.max_connections));
        tokio::pin!(shutdown);

        loop {
            let accepted = tokio::select! {
                () = &mut shutdown => break,
                accepted = accept(&listener, &permits) => accepted,
            };
            let Some((stream, client_addr, permit)) = accepted else {
                tokio::time::sleep(ACCEPT_ERROR_BACKOFF).await;
                continue;
            };
            self.spawn_connection(&graceful, stream, client_addr, permit);
        }

        tracing::info!("proxy shutting down, draining connections");
        tokio::select! {
            () = graceful.shutdown() => tracing::info!("proxy drained"),
            () = tokio::time::sleep(self.config.shutdown_grace) => {
                tracing::warn!("proxy shutdown grace period elapsed with open connections");
            }
        }
    }

    fn spawn_connection(
        &self,
        graceful: &GracefulShutdown,
        stream: TcpStream,
        client_addr: SocketAddr,
        permit: OwnedSemaphorePermit,
    ) {
        let forwarder = Arc::clone(&self.forwarder);
        let service = service_fn(move |request| {
            let forwarder = Arc::clone(&forwarder);
            async move { Ok::<_, Infallible>(forwarder.handle(request, client_addr).await) }
        });
        let connection = http1::Builder::new()
            .timer(TokioTimer::new())
            .header_read_timeout(self.config.header_read_timeout)
            .max_buf_size(self.config.max_header_bytes)
            .serve_connection(TokioIo::new(stream), service);
        let connection = graceful.watch(connection);
        tokio::spawn(async move {
            if let Err(err) = connection.await {
                tracing::debug!(%client_addr, error = %err, "connection closed with error");
            }
            drop(permit);
        });
    }
}

async fn accept(
    listener: &TcpListener,
    permits: &Arc<Semaphore>,
) -> Option<(TcpStream, SocketAddr, OwnedSemaphorePermit)> {
    let permit = Arc::clone(permits).acquire_owned().await.ok()?;
    match listener.accept().await {
        Ok((stream, addr)) => {
            if let Err(err) = stream.set_nodelay(true) {
                tracing::debug!(error = %err, "failed to set TCP_NODELAY");
            }
            Some((stream, addr, permit))
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to accept connection");
            None
        }
    }
}
