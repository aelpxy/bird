use std::collections::BTreeMap;

use serde::Deserialize;

use super::{ContainerInfo, ContainerState, PublishedPort};

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct Inspect {
    id: String,
    name: String,
    state: InspectState,
    config: InspectConfig,
    network_settings: InspectNetwork,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InspectState {
    status: String,
    #[serde(default, rename = "OOMKilled")]
    oom_killed: bool,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InspectConfig {
    #[serde(default)]
    labels: Option<BTreeMap<String, String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InspectNetwork {
    #[serde(default)]
    ports: Option<BTreeMap<String, Option<Vec<InspectBinding>>>>,
    #[serde(default)]
    networks: Option<BTreeMap<String, InspectAttachment>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InspectBinding {
    host_ip: String,
    host_port: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InspectAttachment {
    #[serde(default)]
    aliases: Option<Vec<String>>,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct ListEntry {
    id: String,
    names: Vec<String>,
    state: String,
    #[serde(default)]
    labels: Option<BTreeMap<String, String>>,
    #[serde(default)]
    ports: Option<Vec<ListPort>>,
}

#[derive(Deserialize)]
struct ListPort {
    host_ip: String,
    container_port: u16,
    host_port: u16,
    protocol: String,
}

impl From<Inspect> for ContainerInfo {
    fn from(inspect: Inspect) -> Self {
        let ports = inspect
            .network_settings
            .ports
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(key, bindings)| {
                let container_port = key.strip_suffix("/tcp")?.parse().ok()?;
                Some((container_port, bindings.unwrap_or_default()))
            })
            .flat_map(|(container_port, bindings)| {
                bindings.into_iter().filter_map(move |b| {
                    Some(PublishedPort {
                        container_port,
                        host_port: b.host_port.parse().ok()?,
                        host_ip: b.host_ip,
                    })
                })
            })
            .collect();
        let aliases = inspect
            .network_settings
            .networks
            .unwrap_or_default()
            .into_values()
            .flat_map(|attachment| attachment.aliases.unwrap_or_default())
            .collect();
        Self {
            id: inspect.id,
            name: inspect.name,
            state: ContainerState::from_podman(&inspect.state.status),
            labels: inspect.config.labels.unwrap_or_default(),
            ports,
            aliases,
            oom_killed: inspect.state.oom_killed,
        }
    }
}

impl From<ListEntry> for ContainerInfo {
    fn from(entry: ListEntry) -> Self {
        let ports = entry
            .ports
            .unwrap_or_default()
            .into_iter()
            .filter(|p| p.protocol == "tcp")
            .map(|p| PublishedPort {
                container_port: p.container_port,
                host_ip: p.host_ip,
                host_port: p.host_port,
            })
            .collect();
        Self {
            id: entry.id,
            name: entry.names.into_iter().next().unwrap_or_default(),
            state: ContainerState::from_podman(&entry.state),
            labels: entry.labels.unwrap_or_default(),
            ports,
            aliases: Vec::new(),
            oom_killed: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use bird_core::Port;

    use super::*;

    #[test]
    fn parses_inspect() {
        let raw = r#"{
            "Id": "abc", "Name": "bird-x",
            "State": {"Status": "exited", "OOMKilled": true},
            "Config": {"Labels": {"bird.managed": "true"}},
            "NetworkSettings": {
                "Ports": {
                    "80/tcp": [{"HostIp": "127.0.0.1", "HostPort": "45765"}],
                    "53/udp": [{"HostIp": "127.0.0.1", "HostPort": "5353"}],
                    "443/tcp": null
                },
                "Networks": {"bird": {"IPAddress": "10.89.0.45", "Aliases": ["web", "web.internal", "8fd21b049881"]}}
            }
        }"#;
        let info: ContainerInfo = serde_json::from_str::<Inspect>(raw).unwrap().into();
        assert_eq!(info.state, ContainerState::Exited);
        assert!(info.oom_killed);
        assert_eq!(
            info.labels.get("bird.managed").map(String::as_str),
            Some("true")
        );
        assert_eq!(info.host_port(Port::try_from(80).unwrap()), Some(45765));
        assert_eq!(info.host_port(Port::try_from(443).unwrap()), None);
        assert_eq!(info.ports.len(), 1);
        assert!(info.aliases.contains(&"web.internal".to_owned()));
    }

    #[test]
    fn parses_list_entry() {
        let raw = r#"[{
            "Id": "abc", "Names": ["bird-x"], "State": "exited", "Labels": null,
            "Ports": [{"host_ip": "127.0.0.1", "container_port": 80, "host_port": 45765, "range": 1, "protocol": "tcp"}]
        }]"#;
        let entries: Vec<ListEntry> = serde_json::from_str(raw).unwrap();
        let info = ContainerInfo::from(entries.into_iter().next().unwrap());
        assert_eq!(info.name, "bird-x");
        assert_eq!(info.state, ContainerState::Exited);
        assert!(info.labels.is_empty());
        assert_eq!(info.host_port(Port::try_from(80).unwrap()), Some(45765));
    }
}
