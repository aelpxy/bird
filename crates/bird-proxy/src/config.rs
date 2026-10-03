use std::time::Duration;

#[derive(Debug, Clone)]
pub struct ProxyConfig {
    pub max_connections: usize,
    pub max_header_bytes: usize,
    pub header_read_timeout: Duration,
    pub connect_timeout: Duration,
    pub response_timeout: Duration,
    pub pool_idle_timeout: Duration,
    pub pool_max_idle_per_host: usize,
    pub shutdown_grace: Duration,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            max_connections: 10_000,
            max_header_bytes: 64 * 1024,
            header_read_timeout: Duration::from_secs(10),
            connect_timeout: Duration::from_secs(5),
            response_timeout: Duration::from_secs(60),
            pool_idle_timeout: Duration::from_secs(90),
            pool_max_idle_per_host: 32,
            shutdown_grace: Duration::from_secs(30),
        }
    }
}
