use std::convert::Infallible;
use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use hyper::server::conn::http1;
use hyper::service::service_fn;
use hyper_util::rt::{TokioIo, TokioTimer};
use hyper_util::server::graceful::{GracefulShutdown, Watcher};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};
use tokio_rustls::TlsAcceptor;

use crate::forward::Forwarder;
use crate::headers::forwarded_for;
use crate::scheme::Scheme;
use crate::tls::server_config;
use crate::{ProxyConfig, Routes, Tls};

const ACCEPT_ERROR_BACKOFF: Duration = Duration::from_millis(100);
const TLS_HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

pub struct Proxy {
    forwarder: Arc<Forwarder>,
    tls: Option<Tls>,
    config: ProxyConfig,
}

impl Proxy {
    #[must_use]
    pub fn new(routes: Routes, config: ProxyConfig, tls: Option<Tls>) -> Self {
        Self {
            forwarder: Arc::new(Forwarder::new(routes, tls.clone(), &config)),
            tls,
            config,
        }
    }

    pub async fn serve(&self, listener: TcpListener, shutdown: impl Future<Output = ()>) {
        self.accept_loop(listener, shutdown, None).await;
    }

    pub async fn serve_tls(
        &self,
        listener: TcpListener,
        shutdown: impl Future<Output = ()>,
    ) -> Result<(), rustls::Error> {
        let Some(tls) = &self.tls else {
            return Err(rustls::Error::General(
                "proxy was created without tls settings".to_owned(),
            ));
        };
        let acceptor = TlsAcceptor::from(server_config(tls.certificates.clone())?);
        self.accept_loop(listener, shutdown, Some(acceptor)).await;
        Ok(())
    }

    async fn accept_loop(
        &self,
        listener: TcpListener,
        shutdown: impl Future<Output = ()>,
        acceptor: Option<TlsAcceptor>,
    ) {
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
            let connection = Connection {
                forwarder: Arc::clone(&self.forwarder),
                watcher: graceful.watcher(),
                client_addr,
                header_read_timeout: self.config.header_read_timeout,
                max_header_bytes: self.config.max_header_bytes,
            };
            let acceptor = acceptor.clone();
            tokio::spawn(async move {
                match acceptor {
                    None => connection.serve(stream, Scheme::Http).await,
                    Some(acceptor) => connection.serve_tls(&acceptor, stream).await,
                }
                drop(permit);
            });
        }

        tracing::info!("proxy shutting down, draining connections");
        tokio::select! {
            () = graceful.shutdown() => tracing::info!("proxy drained"),
            () = tokio::time::sleep(self.config.shutdown_grace) => {
                tracing::warn!("proxy shutdown grace period elapsed with open connections");
            }
        }
    }
}

struct Connection {
    forwarder: Arc<Forwarder>,
    watcher: Watcher,
    client_addr: SocketAddr,
    header_read_timeout: Duration,
    max_header_bytes: usize,
}

impl Connection {
    async fn serve_tls(self, acceptor: &TlsAcceptor, stream: TcpStream) {
        match tokio::time::timeout(TLS_HANDSHAKE_TIMEOUT, acceptor.accept(stream)).await {
            Ok(Ok(stream)) => self.serve(stream, Scheme::Https).await,
            Ok(Err(err)) => {
                tracing::debug!(client_addr = %self.client_addr, error = %err, "tls handshake failed");
            }
            Err(_) => tracing::debug!(client_addr = %self.client_addr, "tls handshake timed out"),
        }
    }

    async fn serve<I>(self, io: I, scheme: Scheme)
    where
        I: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let forwarder = self.forwarder;
        let forwarded_for = forwarded_for(self.client_addr.ip());
        let service = service_fn(move |request| {
            let forwarder = Arc::clone(&forwarder);
            let forwarded_for = forwarded_for.clone();
            async move { Ok::<_, Infallible>(forwarder.handle(request, forwarded_for, scheme).await) }
        });
        let connection = http1::Builder::new()
            .timer(TokioTimer::new())
            .header_read_timeout(self.header_read_timeout)
            .max_buf_size(self.max_header_bytes)
            .serve_connection(TokioIo::new(io), service);
        if let Err(err) = self.watcher.watch(connection).await {
            tracing::debug!(client_addr = %self.client_addr, error = %err, "connection closed with error");
        }
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
