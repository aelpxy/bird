mod support;

use std::time::Duration;

use bird_proxy::ProxyConfig;
use hyper::StatusCode;
use support::{get, routes, spawn_proxy, spawn_upstream};
use tokio::net::TcpStream;

#[tokio::test]
async fn drains_in_flight_requests() {
    let upstream = spawn_upstream("a").await;
    let mut proxy = spawn_proxy(
        routes(&[("web.localhost", &[upstream])]),
        ProxyConfig::default(),
    )
    .await;
    let addr = proxy.addr;

    let in_flight = tokio::spawn(async move { get(addr, "web.localhost", "/slow").await });
    tokio::time::sleep(Duration::from_millis(100)).await;
    proxy.signal_shutdown();

    let reply = in_flight.await.unwrap();
    assert_eq!(reply.status, StatusCode::OK);
    assert!(reply.text().contains("slow=done"));
    proxy.join().await;

    assert!(TcpStream::connect(addr).await.is_err());
}
