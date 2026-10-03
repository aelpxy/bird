use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use bird_core::HealthCheck;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const TCP_SETTLE: Duration = Duration::from_millis(300);
const PROBE_REQUEST: &[u8] = b"GET / HTTP/1.0\r\nHost: localhost\r\n\r\n";

pub(crate) async fn probe(check: HealthCheck, address: SocketAddr) -> bool {
    match check {
        HealthCheck::Http => responds_to_http(address).await,
        HealthCheck::Tcp => accepts_tcp(address).await,
    }
}

// a bare tcp connect is not enough: rootless port forwarding accepts before the app listens
async fn responds_to_http(address: SocketAddr) -> bool {
    let probe = async {
        let mut stream = TcpStream::connect(address).await.ok()?;
        stream.write_all(PROBE_REQUEST).await.ok()?;
        let mut prefix = [0_u8; 5];
        stream.read_exact(&mut prefix).await.ok()?;
        Some(&prefix == b"HTTP/")
    };
    tokio::time::timeout(PROBE_TIMEOUT, probe)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

// rootless port forwarding accepts every connect, then resets it at once when nothing listens
// behind it, so a connection that stays open or sends data means the app is really there
async fn accepts_tcp(address: SocketAddr) -> bool {
    let probe = async {
        let mut stream = TcpStream::connect(address).await.ok()?;
        let mut byte = [0_u8; 1];
        match tokio::time::timeout(TCP_SETTLE, stream.read(&mut byte)).await {
            Err(_) | Ok(Ok(1..)) => Some(true),
            Ok(Ok(0) | Err(_)) => Some(false),
        }
    };
    tokio::time::timeout(PROBE_TIMEOUT, probe)
        .await
        .ok()
        .flatten()
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use tokio::net::TcpListener;

    use super::*;

    async fn server(reply: &'static [u8]) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0_u8; 256];
                let _ = stream.read(&mut buf).await;
                let _ = stream.write_all(reply).await;
            }
        });
        address
    }

    #[tokio::test]
    async fn tcp_check_accepts_silent_and_talkative_servers() {
        let silent = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let silent_address = silent.local_addr().unwrap();
        tokio::spawn(async move {
            let mut held = Vec::new();
            while let Ok((stream, _)) = silent.accept().await {
                held.push(stream);
            }
        });
        assert!(probe(HealthCheck::Tcp, silent_address).await);

        let greeting = server(b"+OK ready\r\n").await;
        assert!(probe(HealthCheck::Tcp, greeting).await);
    }

    #[tokio::test]
    async fn tcp_check_rejects_closed_or_dropping_ports() {
        let dropping = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dropping_address = dropping.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((stream, _)) = dropping.accept().await {
                drop(stream);
            }
        });
        assert!(!probe(HealthCheck::Tcp, dropping_address).await);

        let closed = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        assert!(!probe(HealthCheck::Tcp, closed).await);
    }

    #[tokio::test]
    async fn accepts_any_http_status() {
        let address = server(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
        assert!(responds_to_http(address).await);
    }

    #[tokio::test]
    async fn rejects_non_http_and_closed_ports() {
        let address = server(b"SSH-2.0-OpenSSH\r\n").await;
        assert!(!responds_to_http(address).await);

        let closed = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        assert!(!responds_to_http(closed).await);
    }
}
