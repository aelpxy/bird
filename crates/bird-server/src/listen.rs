use std::io;
use std::net::SocketAddr;

use tokio::net::TcpListener;

use crate::{Error, Result};

const FIRST_UNPRIVILEGED_PORT: u16 = 1024;

pub(crate) fn bind(addr: SocketAddr) -> Result<TcpListener> {
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
