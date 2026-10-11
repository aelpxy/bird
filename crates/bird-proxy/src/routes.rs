use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use arc_swap::ArcSwap;
use bird_core::{Hostname, MachineId};
use hyper::header::HeaderValue;
use hyper::http::uri::Authority;

use crate::policy::RoutePolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RouteError {
    UnknownHost,
    NoUpstreams,
}

#[derive(Debug, Default)]
struct Backend {
    upstreams: Vec<Upstream>,
    next: AtomicUsize,
    policy: Option<Arc<RoutePolicy>>,
    redirect_to: Option<Hostname>,
}

#[derive(Debug, Clone)]
struct Upstream {
    authority: Authority,
    // sent back as x-machine-id, so every answer says which machine served it
    machine: Option<HeaderValue>,
}

/// Everything the proxy needs to serve one hostname
#[derive(Debug, Default)]
pub struct Route {
    pub upstreams: Vec<(SocketAddr, Option<MachineId>)>,
    pub policy: Option<Arc<RoutePolicy>>,
    /// Answer with a redirect to this hostname instead of proxying
    pub redirect_to: Option<Hostname>,
}

/// Where a request for a hostname goes
#[derive(Debug)]
pub(crate) enum Target {
    Upstream {
        authority: Authority,
        machine: Option<HeaderValue>,
        policy: Option<Arc<RoutePolicy>>,
    },
    Redirect(Hostname),
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
        let route = Route {
            upstreams: upstreams.iter().map(|&addr| (addr, None)).collect(),
            ..Route::default()
        };
        self.insert_route(host, route);
    }

    pub fn insert_route(&mut self, host: &Hostname, route: Route) {
        let upstreams = route
            .upstreams
            .iter()
            .filter_map(|(addr, machine)| {
                Some(Upstream {
                    authority: Authority::try_from(addr.to_string()).ok()?,
                    machine: machine
                        .and_then(|machine| HeaderValue::try_from(machine.to_string()).ok()),
                })
            })
            .collect();
        self.backends.insert(
            host.as_str().to_owned(),
            Backend {
                upstreams,
                next: AtomicUsize::new(0),
                policy: route.policy,
                redirect_to: route.redirect_to,
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

    fn pick(&self, host: &str) -> Result<Target, RouteError> {
        let backend = self.backends.get(host).ok_or(RouteError::UnknownHost)?;
        if let Some(target) = &backend.redirect_to {
            return Ok(Target::Redirect(target.clone()));
        }
        let count = backend.upstreams.len();
        if count == 0 {
            return Err(RouteError::NoUpstreams);
        }
        let index = backend.next.fetch_add(1, Ordering::Relaxed) % count;
        let upstream = backend
            .upstreams
            .get(index)
            .ok_or(RouteError::NoUpstreams)?;
        Ok(Target::Upstream {
            authority: upstream.authority.clone(),
            machine: upstream.machine.clone(),
            policy: backend.policy.clone(),
        })
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

    pub(crate) fn pick(&self, host: &str) -> Result<Target, RouteError> {
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

    fn upstream(target: Result<Target, RouteError>) -> String {
        match target.unwrap() {
            Target::Upstream { authority, .. } => authority.to_string(),
            Target::Redirect(to) => panic!("redirected to {to}"),
        }
    }

    #[test]
    fn round_robins_across_upstreams() {
        let mut table = RouteTable::new();
        table.insert(&host("web.localhost"), &[addr(1001), addr(1002)]);
        let picks: Vec<String> = (0..4)
            .map(|_| upstream(table.pick("web.localhost")))
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
        assert_eq!(upstream(routes.pick("web.localhost")), "127.0.0.1:1001");
    }

    #[test]
    fn redirects_and_names_the_machine() {
        let mut table = RouteTable::new();
        let machine = MachineId::generate();
        table.insert_route(
            &host("example.com"),
            Route {
                upstreams: vec![(addr(1001), Some(machine))],
                ..Route::default()
            },
        );
        table.insert_route(
            &host("www.example.com"),
            Route {
                redirect_to: Some(host("example.com")),
                ..Route::default()
            },
        );
        match table.pick("example.com").unwrap() {
            Target::Upstream {
                machine: header, ..
            } => {
                assert_eq!(header.unwrap(), machine.to_string().as_str());
            }
            Target::Redirect(to) => panic!("redirected to {to}"),
        }
        assert!(matches!(
            table.pick("www.example.com").unwrap(),
            Target::Redirect(to) if to.as_str() == "example.com"
        ));
    }
}
