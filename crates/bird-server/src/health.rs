use std::net::SocketAddr;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use bird_core::HealthCheck;

const PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const TCP_SETTLE: Duration = Duration::from_millis(300);
// "HTTP/1.1 200", the shortest prefix that holds the status code
const STATUS_LINE_BYTES: usize = 12;

pub(crate) async fn probe(check: &HealthCheck, address: SocketAddr) -> bool {
    match check {
        HealthCheck::Http => http_status(address, "/").await.is_some(),
        HealthCheck::Path(path) => http_status(address, path.as_str())
            .await
            .is_some_and(|status| (200..300).contains(&status)),
        HealthCheck::Tcp => accepts_tcp(address).await,
    }
}

// finishes sentences like "the app did not ..." in deploy errors
pub(crate) fn expectation(check: &HealthCheck) -> String {
    match check {
        HealthCheck::Http => "answer http".to_owned(),
        HealthCheck::Path(path) => format!("answer {path} with a 2xx status"),
        HealthCheck::Tcp => "accept tcp connections".to_owned(),
    }
}

// a bare tcp connect is not enough: rootless port forwarding accepts before the app listens
async fn http_status(address: SocketAddr, path: &str) -> Option<u16> {
    let probe = async {
        let mut stream = TcpStream::connect(address).await.ok()?;
        let request = format!("GET {path} HTTP/1.0\r\nHost: localhost\r\n\r\n");
        stream.write_all(request.as_bytes()).await.ok()?;
        let mut line = [0_u8; STATUS_LINE_BYTES];
        stream.read_exact(&mut line).await.ok()?;
        parse_status(&line)
    };
    tokio::time::timeout(PROBE_TIMEOUT, probe)
        .await
        .ok()
        .flatten()
}

fn parse_status(line: &[u8; STATUS_LINE_BYTES]) -> Option<u16> {
    let rest = line.strip_prefix(b"HTTP/1.")?;
    let (_minor, rest) = rest.split_first()?;
    let digits = rest.strip_prefix(b" ")?;
    if !digits.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(digits).ok()?.parse().ok()
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
        routed(move |_| reply).await
    }

    // replies by request line, so a test sees which path the probe asked for
    async fn routed(reply: impl Fn(&str) -> &'static [u8] + Send + 'static) -> SocketAddr {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let mut buf = [0_u8; 256];
                let read = stream.read(&mut buf).await.unwrap_or(0);
                let request = String::from_utf8_lossy(&buf[..read]).into_owned();
                let first = request.lines().next().unwrap_or_default().to_owned();
                let _ = stream.write_all(reply(&first)).await;
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
        assert!(probe(&HealthCheck::Tcp, silent_address).await);

        let greeting = server(b"+OK ready\r\n").await;
        assert!(probe(&HealthCheck::Tcp, greeting).await);
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
        assert!(!probe(&HealthCheck::Tcp, dropping_address).await);

        let closed = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        assert!(!probe(&HealthCheck::Tcp, closed).await);
    }

    #[tokio::test]
    async fn accepts_any_http_status() {
        let address = server(b"HTTP/1.1 404 Not Found\r\n\r\n").await;
        assert!(probe(&HealthCheck::Http, address).await);
        let old = server(b"HTTP/1.0 500 Internal Server Error\r\n\r\n").await;
        assert!(probe(&HealthCheck::Http, old).await);
    }

    #[tokio::test]
    async fn path_check_needs_a_2xx_on_that_path() {
        let address = routed(|line| match line {
            "GET /healthz HTTP/1.0" => b"HTTP/1.1 204 No Content\r\n\r\n",
            "GET /ready?deep=1 HTTP/1.0" => b"HTTP/1.1 503 Service Unavailable\r\n\r\n",
            "GET /moved HTTP/1.0" => b"HTTP/1.1 301 Moved Permanently\r\n\r\n",
            _ => b"HTTP/1.1 404 Not Found\r\n\r\n",
        })
        .await;
        let check = |path: &str| path.parse::<HealthCheck>().unwrap();
        assert!(probe(&check("/healthz"), address).await);
        for failing in ["/ready?deep=1", "/moved", "/missing"] {
            assert!(!probe(&check(failing), address).await, "{failing}");
        }
    }

    #[test]
    fn parses_status_lines() {
        assert_eq!(parse_status(b"HTTP/1.1 200"), Some(200));
        assert_eq!(parse_status(b"HTTP/1.0 503"), Some(503));
        for bad in [
            b"HTTP/2.0 200",
            b"HTTP/1.1 2x0",
            b"HTTP/1.1-200",
            b"SSH-2.0-Open",
        ] {
            assert_eq!(parse_status(bad), None);
        }
    }

    #[tokio::test]
    async fn rejects_non_http_and_closed_ports() {
        let address = server(b"SSH-2.0-OpenSSH\r\n").await;
        assert!(!probe(&HealthCheck::Http, address).await);

        let closed = TcpListener::bind("127.0.0.1:0")
            .await
            .unwrap()
            .local_addr()
            .unwrap();
        assert!(!probe(&HealthCheck::Http, closed).await);
    }
}
