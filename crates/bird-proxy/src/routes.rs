use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arc_swap::ArcSwap;
use bird_core::Hostname;
use hyper::http::uri::Authority;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteError {
    UnknownHost,
    NoUpstreams,
}

#[derive(Debug, Default)]
struct Backend {
    upstreams: Vec<Authority>,
    next: AtomicUsize,
}

#[derive(Debug, Default)]
pub struct RouteTable {
    backends: HashMap<String, Backend>,
}

impl RouteTable {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert(&mut self, host: &Hostname, upstreams: &[SocketAddr]) {
        let upstreams = upstreams
            .iter()
            .filter_map(|addr| Authority::try_from(addr.to_string()).ok())
            .collect();
        self.backends.insert(
            host.as_str().to_owned(),
            Backend {
                upstreams,
                next: AtomicUsize::new(0),
            },
        );
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.backends.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.backends.is_empty()
    }

    fn pick(&self, host: &str) -> Result<Authority, RouteError> {
        let backend = self.backends.get(host).ok_or(RouteError::UnknownHost)?;
        let count = backend.upstreams.len();
        if count == 0 {
            return Err(RouteError::NoUpstreams);
        }
        let index = backend.next.fetch_add(1, Ordering::Relaxed) % count;
        backend
            .upstreams
            .get(index)
            .cloned()
            .ok_or(RouteError::NoUpstreams)
    }
}

#[derive(Debug, Clone, Default)]
pub struct Routes {
    table: Arc<ArcSwap<RouteTable>>,
}

impl Routes {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn replace(&self, table: RouteTable) {
        self.table.store(Arc::new(table));
    }

    pub(crate) fn pick(&self, host: &str) -> Result<Authority, RouteError> {
        self.table.load().pick(host)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn addr(port: u16) -> SocketAddr {
        SocketAddr::from(([127, 0, 0, 1], port))
    }

    fn host(raw: &str) -> Hostname {
        raw.parse().unwrap()
    }

    #[test]
    fn round_robins_across_upstreams() {
        let mut table = RouteTable::new();
        table.insert(&host("web.localhost"), &[addr(1001), addr(1002)]);
        let picks: Vec<String> = (0..4)
            .map(|_| table.pick("web.localhost").unwrap().to_string())
            .collect();
        assert_eq!(
            picks,
            [
                "127.0.0.1:1001",
                "127.0.0.1:1002",
                "127.0.0.1:1001",
                "127.0.0.1:1002"
            ]
        );
    }

    #[test]
    fn distinguishes_unknown_host_from_empty_backend() {
        let mut table = RouteTable::new();
        table.insert(&host("web.localhost"), &[]);
        assert_eq!(
            table.pick("api.localhost").unwrap_err(),
            RouteError::UnknownHost
        );
        assert_eq!(
            table.pick("web.localhost").unwrap_err(),
            RouteError::NoUpstreams
        );
    }

    #[test]
    fn replace_swaps_table() {
        let routes = Routes::new();
        assert_eq!(
            routes.pick("web.localhost").unwrap_err(),
            RouteError::UnknownHost
        );
        let mut table = RouteTable::new();
        table.insert(&host("web.localhost"), &[addr(1001)]);
        routes.replace(table);
        assert_eq!(
            routes.pick("web.localhost").unwrap().as_str(),
            "127.0.0.1:1001"
        );
    }
}
