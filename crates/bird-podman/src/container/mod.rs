mod inspect;
mod spec;

use std::collections::BTreeMap;
use std::time::Duration;

use bird_core::{EnvKey, ImageRef, Port};
use hyper::Method;
use serde::Deserialize;

use crate::client::DEFAULT_TIMEOUT;
use crate::error::check;
use crate::query::encode;
use crate::{Podman, Result, VolumeMount};
use inspect::{Inspect, ListEntry};
use spec::SpecGenerator;

#[derive(Debug, Clone)]
pub struct ContainerSpec {
    pub name: String,
    pub image: ImageRef,
    pub port: Port,
    pub network: String,
    pub aliases: Vec<String>,
    pub mounts: Vec<VolumeMount>,
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
    pub aliases: Vec<String>,
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

#[derive(Deserialize)]
struct CreateResponse {
    #[serde(rename = "Id")]
    id: String,
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
