use std::io;
use std::net::SocketAddr;

use tokio::net::{TcpListener, TcpSocket};

// matches the usual net.core.somaxconn; the default of 128 overflows under connection bursts
const BACKLOG: u32 = 4096;

pub fn bind(addr: SocketAddr) -> io::Result<TcpListener> {
    let socket = if addr.is_ipv4() {
        TcpSocket::new_v4()?
    } else {
        TcpSocket::new_v6()?
    };
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;
    socket.listen(BACKLOG)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn binds_and_accepts() {
        let listener = bind("127.0.0.1:0".parse().unwrap()).unwrap();
        let addr = listener.local_addr().unwrap();
        let client = tokio::net::TcpStream::connect(addr);
        let (accepted, connected) = tokio::join!(listener.accept(), client);
        assert!(accepted.is_ok());
        assert!(connected.is_ok());
    }
}
