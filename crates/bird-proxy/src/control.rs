use std::net::SocketAddr;
use std::time::Duration;

use bird_core::Hostname;

/// birdd's own api at a domain; slow deploys, big uploads and quiet streams need limits of their own
#[derive(Debug, Clone)]
pub struct ControlRoute {
    pub host: Hostname,
    pub upstream: SocketAddr,
    pub response_timeout: Duration,
    pub body_idle_timeout: Duration,
    pub max_body_bytes: usize,
}
