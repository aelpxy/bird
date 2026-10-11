use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, PoisonError};

use bird_core::{Hostname, ProxySettings, ServiceId};
use bird_proxy::{Route, RoutePolicy, RouteTable};
use bird_store::RouteEntry;

use crate::Result;
use crate::state::AppState;

// one policy per service, reused while its settings stay the same so rate limits keep counting
#[derive(Debug, Default)]
pub(crate) struct Policies(Mutex<HashMap<ServiceId, (ProxySettings, Arc<RoutePolicy>)>>);

impl Policies {
    fn current(&self, entries: &[RouteEntry]) -> HashMap<ServiceId, Arc<RoutePolicy>> {
        let mut kept = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mut wanted: HashMap<ServiceId, &ProxySettings> = HashMap::new();
        for entry in entries.iter().filter(|entry| !entry.proxy.is_default()) {
            wanted.insert(entry.service_id, &entry.proxy);
        }
        kept.retain(|service, (settings, _)| wanted.get(service) == Some(&&*settings));
        wanted
            .into_iter()
            .map(|(service, settings)| {
                let (_, policy) = kept
                    .entry(service)
                    .or_insert_with(|| (settings.clone(), Arc::new(RoutePolicy::new(settings))));
                (service, Arc::clone(policy))
            })
            .collect()
    }
}

pub(crate) async fn refresh(state: &AppState) -> Result<()> {
    let entries = state.db.call(|store| store.list_routes()).await?;
    let policies = state.policies.current(&entries);
    let table = build_table(entries, &policies);
    tracing::debug!(hosts = table.len(), "routes refreshed");
    state.routes.replace(table);
    Ok(())
}

fn build_table(
    entries: Vec<RouteEntry>,
    policies: &HashMap<ServiceId, Arc<RoutePolicy>>,
) -> RouteTable {
    let mut grouped: BTreeMap<Hostname, Route> = BTreeMap::new();
    for entry in entries {
        let route = grouped.entry(entry.hostname).or_insert_with(|| Route {
            policy: policies.get(&entry.service_id).cloned(),
            redirect_to: entry.redirect_to,
            ..Route::default()
        });
        route
            .upstreams
            .extend(entry.address.map(|address| (address, entry.machine_id)));
    }
    let mut table = RouteTable::new();
    for (hostname, route) in grouped {
        table.insert_route(&hostname, route);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(host: &str, address: Option<&str>, service: ServiceId, proxy: &str) -> RouteEntry {
        RouteEntry {
            hostname: host.parse().unwrap(),
            address: address.map(|a| a.parse().unwrap()),
            machine_id: None,
            service_id: service,
            proxy: serde_json::from_str(proxy).unwrap(),
            redirect_to: None,
        }
    }

    #[test]
    fn groups_entries_per_host() {
        let service = ServiceId::generate();
        let table = build_table(
            vec![
                entry("a.localhost", Some("127.0.0.1:1"), service, "{}"),
                entry("a.localhost", Some("127.0.0.1:2"), service, "{}"),
                entry("b.localhost", None, service, "{}"),
            ],
            &HashMap::new(),
        );
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn keeps_a_policy_until_its_settings_change() {
        let policies = Policies::default();
        let service = ServiceId::generate();
        let limited = r#"{"rate_limit":{"per_second":1,"burst":1}}"#;
        let first = policies.current(&[entry("a.localhost", None, service, limited)]);
        let again = policies.current(&[entry("a.localhost", None, service, limited)]);
        assert!(Arc::ptr_eq(&first[&service], &again[&service]));

        let changed = policies.current(&[entry(
            "a.localhost",
            None,
            service,
            r#"{"maintenance":true}"#,
        )]);
        assert!(!Arc::ptr_eq(&first[&service], &changed[&service]));
        assert!(
            policies
                .current(&[entry("a.localhost", None, service, "{}")])
                .is_empty()
        );
    }
}
