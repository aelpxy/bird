use std::io;
use std::net::SocketAddr;

use tokio::net::TcpListener;

use crate::{Error, Result};

const FIRST_UNPRIVILEGED_PORT: u16 = 1024;

/// Sockets handed over by systemd (`birdd.socket`), which keeps them open while birdd restarts, so
/// connections wait in the kernel's queue instead of being refused during an upgrade
pub(crate) struct Inherited(Vec<std::net::TcpListener>);

impl Inherited {
    pub(crate) fn from_env() -> Self {
        let mut fds = listenfd::ListenFd::from_env();
        let listeners = (0..fds.len())
            .filter_map(|index| match fds.take_tcp_listener(index) {
                Ok(listener) => listener,
                Err(err) => {
                    tracing::warn!(index, error = %err, "ignoring a socket from systemd");
                    None
                }
            })
            .collect();
        Self(listeners)
    }

    /// The socket systemd holds for `addr`, else a fresh one
    pub(crate) fn take(&mut self, addr: SocketAddr) -> Result<TcpListener> {
        let held = self
            .0
            .iter()
            .position(|listener| listener.local_addr().is_ok_and(|local| local == addr));
        let Some(index) = held else {
            return bind(addr);
        };
        let listener = self.0.swap_remove(index);
        listener.set_nonblocking(true)?;
        tracing::info!(%addr, "listening on the socket systemd holds");
        Ok(TcpListener::from_std(listener)?)
    }

    pub(crate) fn warn_unused(self) {
        for listener in self.0 {
            if let Ok(addr) = listener.local_addr() {
                tracing::warn!(%addr, "systemd passed a socket birdd does not listen on, check birdd.socket");
            }
        }
    }
}

fn bind(addr: SocketAddr) -> Result<TcpListener> {
    bird_proxy::bind(addr).map_err(|err| classify(addr, err))
}

fn classify(addr: SocketAddr, source: io::Error) -> Error {
    if source.kind() == io::ErrorKind::PermissionDenied && addr.port() < FIRST_UNPRIVILEGED_PORT {
        Error::PrivilegedPort(addr)
    } else {
        Error::Bind { addr, source }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explains_privileged_ports() {
        let denied = || io::Error::from(io::ErrorKind::PermissionDenied);
        assert!(matches!(
            classify("0.0.0.0:80".parse().unwrap(), denied()),
            Error::PrivilegedPort(_)
        ));
        assert!(matches!(
            classify("0.0.0.0:8080".parse().unwrap(), denied()),
            Error::Bind { .. }
        ));
        let in_use = classify(
            "0.0.0.0:443".parse().unwrap(),
            io::Error::from(io::ErrorKind::AddrInUse),
        );
        assert!(in_use.to_string().contains("0.0.0.0:443"));
    }
}
