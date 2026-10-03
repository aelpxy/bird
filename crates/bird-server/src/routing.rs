use std::collections::BTreeMap;
use std::net::SocketAddr;

use bird_core::Hostname;
use bird_proxy::RouteTable;
use bird_store::RouteEntry;

use crate::Result;
use crate::state::AppState;

pub(crate) async fn refresh(state: &AppState) -> Result<()> {
    let entries = state.db.call(|store| store.list_routes()).await?;
    let table = build_table(entries);
    tracing::debug!(hosts = table.len(), "routes refreshed");
    state.routes.replace(table);
    Ok(())
}

fn build_table(entries: Vec<RouteEntry>) -> RouteTable {
    let mut grouped: BTreeMap<Hostname, Vec<SocketAddr>> = BTreeMap::new();
    for entry in entries {
        let upstreams = grouped.entry(entry.hostname).or_default();
        upstreams.extend(entry.address);
    }
    let mut table = RouteTable::new();
    for (hostname, upstreams) in &grouped {
        table.insert(hostname, upstreams);
    }
    table
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_entries_per_host() {
        let entry = |host: &str, address: Option<&str>| RouteEntry {
            hostname: host.parse().unwrap(),
            address: address.map(|a| a.parse().unwrap()),
        };
        let table = build_table(vec![
            entry("a.localhost", Some("127.0.0.1:1")),
            entry("a.localhost", Some("127.0.0.1:2")),
            entry("b.localhost", None),
        ]);
        assert_eq!(table.len(), 2);
    }
}
