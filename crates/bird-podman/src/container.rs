use std::collections::BTreeMap;
use std::time::Duration;

use bird_core::{EnvKey, ImageRef, Port};
use hyper::Method;
use serde::{Deserialize, Serialize};

use crate::client::DEFAULT_TIMEOUT;
use crate::error::check;
use crate::image::qualify_image;
use crate::query::encode;
use crate::{Podman, Result};

#[derive(Debug, Clone)]
pub struct ContainerSpec {
    pub name: String,
    pub image: ImageRef,
    pub port: Port,
    pub network: String,
    pub env: BTreeMap<EnvKey, String>,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Stopping,
    Exited,
    Removing,
    Unknown,
}

impl ContainerState {
    fn from_podman(status: &str) -> Self {
        match status {
            "created" | "configured" | "initialized" => Self::Created,
            "running" => Self::Running,
            "paused" => Self::Paused,
            "stopping" => Self::Stopping,
            "exited" | "stopped" => Self::Exited,
            "removing" => Self::Removing,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PublishedPort {
    pub container_port: u16,
    pub host_ip: String,
    pub host_port: u16,
}

#[derive(Debug, Clone)]
pub struct ContainerInfo {
    pub id: String,
    pub name: String,
    pub state: ContainerState,
    pub labels: BTreeMap<String, String>,
    pub ports: Vec<PublishedPort>,
}

impl ContainerInfo {
    #[must_use]
    pub fn host_port(&self, container_port: Port) -> Option<u16> {
        self.ports
            .iter()
            .find(|p| p.container_port == container_port.get())
            .map(|p| p.host_port)
    }
}

#[derive(Serialize)]
struct SpecGenerator<'a> {
    name: &'a str,
    image: String,
    env: &'a BTreeMap<EnvKey, String>,
    labels: &'a BTreeMap<String, String>,
    netns: Namespace,
    networks: BTreeMap<&'a str, serde_json::Map<String, serde_json::Value>>,
    portmappings: [PortMapping; 1],
    restart_policy: &'static str,
}

#[derive(Serialize)]
struct Namespace {
    nsmode: &'static str,
}

#[derive(Serialize)]
struct PortMapping {
    container_port: u16,
    host_ip: &'static str,
    protocol: &'static str,
}

#[derive(Deserialize)]
struct CreateResponse {
    #[serde(rename = "Id")]
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct Inspect {
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
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InspectBinding {
    host_ip: String,
    host_port: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ListEntry {
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
        Self {
            id: inspect.id,
            name: inspect.name,
            state: ContainerState::from_podman(&inspect.state.status),
            labels: inspect.config.labels.unwrap_or_default(),
            ports,
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
        }
    }
}

impl<'a> From<&'a ContainerSpec> for SpecGenerator<'a> {
    fn from(spec: &'a ContainerSpec) -> Self {
        Self {
            name: &spec.name,
            image: qualify_image(&spec.image),
            env: &spec.env,
            labels: &spec.labels,
            netns: Namespace { nsmode: "bridge" },
            networks: BTreeMap::from([(spec.network.as_str(), serde_json::Map::new())]),
            portmappings: [PortMapping {
                container_port: spec.port.get(),
                host_ip: "127.0.0.1",
                protocol: "tcp",
            }],
            restart_policy: "unless-stopped",
        }
    }
}

impl Podman {
    pub async fn create_container(&self, spec: &ContainerSpec) -> Result<String> {
        let request = SpecGenerator::from(spec);
        let response = self.post_json("/containers/create", &request).await?;
        let body = check(response, || format!("container {}", spec.name))?;
        Ok(serde_json::from_slice::<CreateResponse>(&body)?.id)
    }

    pub async fn start_container(&self, id: &str) -> Result<()> {
        let response = self
            .send(
                Method::POST,
                &format!("/containers/{}/start", encode(id)),
                DEFAULT_TIMEOUT,
            )
            .await?;
        check(response, || format!("container {id}"))?;
        Ok(())
    }

    pub async fn stop_container(&self, id: &str, grace: Duration) -> Result<()> {
        let path = format!(
            "/containers/{}/stop?timeout={}",
            encode(id),
            grace.as_secs()
        );
        let response = self
            .send(Method::POST, &path, grace + DEFAULT_TIMEOUT)
            .await?;
        check(response, || format!("container {id}"))?;
        Ok(())
    }

    pub async fn remove_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}?force=true", encode(id));
        let response = self.send(Method::DELETE, &path, DEFAULT_TIMEOUT).await?;
        check(response, || format!("container {id}"))?;
        Ok(())
    }

    pub async fn inspect_container(&self, id: &str) -> Result<ContainerInfo> {
        let response = self
            .get(&format!("/containers/{}/json", encode(id)))
            .await?;
        let body = check(response, || format!("container {id}"))?;
        Ok(serde_json::from_slice::<Inspect>(&body)?.into())
    }

    pub async fn list_containers(&self, label: &str) -> Result<Vec<ContainerInfo>> {
        let filters = serde_json::json!({ "label": [label] }).to_string();
        let path = format!("/containers/json?all=true&filters={}", encode(&filters));
        let response = self.get(&path).await?;
        let body = check(response, || "containers".to_owned())?;
        let entries: Vec<ListEntry> = serde_json::from_slice(&body)?;
        Ok(entries.into_iter().map(ContainerInfo::from).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_inspect() {
        let raw = r#"{
            "Id": "abc", "Name": "bird-x",
            "State": {"Status": "running"},
            "Config": {"Labels": {"bird.managed": "true"}},
            "NetworkSettings": {"Ports": {
                "80/tcp": [{"HostIp": "127.0.0.1", "HostPort": "45765"}],
                "53/udp": [{"HostIp": "127.0.0.1", "HostPort": "5353"}],
                "443/tcp": null
            }}
        }"#;
        let info: ContainerInfo = serde_json::from_str::<Inspect>(raw).unwrap().into();
        assert_eq!(info.state, ContainerState::Running);
        assert_eq!(
            info.labels.get("bird.managed").map(String::as_str),
            Some("true")
        );
        assert_eq!(info.host_port(Port::try_from(80).unwrap()), Some(45765));
        assert_eq!(info.host_port(Port::try_from(443).unwrap()), None);
        assert_eq!(info.ports.len(), 1);
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

    #[test]
    fn serializes_spec() {
        let spec = ContainerSpec {
            name: "bird-x".to_owned(),
            image: "nginx:alpine".parse().unwrap(),
            port: Port::try_from(80).unwrap(),
            network: "bird".to_owned(),
            env: BTreeMap::from([("A".parse().unwrap(), "b".to_owned())]),
            labels: BTreeMap::new(),
        };
        let json = serde_json::to_value(SpecGenerator::from(&spec)).unwrap();
        assert_eq!(json["image"], "docker.io/library/nginx:alpine");
        assert_eq!(json["env"]["A"], "b");
        assert_eq!(json["networks"]["bird"], serde_json::json!({}));
        assert_eq!(json["netns"]["nsmode"], "bridge");
        assert_eq!(json["portmappings"][0]["container_port"], 80);
        assert_eq!(json["portmappings"][0]["host_ip"], "127.0.0.1");
    }
}
