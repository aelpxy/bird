mod inspect;
mod spec;

use std::collections::BTreeMap;
use std::time::Duration;

use bird_core::{EnvKey, ImageRef, Port};
use hyper::Method;
use hyper::upgrade::Upgraded;
use hyper_util::rt::TokioIo;
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
    pub command: Option<Vec<String>>,
    // replaces the image's entrypoint; the image's command is then dropped too
    pub entrypoint: Option<Vec<String>>,
    pub lifecycle: Lifecycle,
    pub network: String,
    pub aliases: Vec<String>,
    pub mounts: Vec<VolumeMount>,
    pub limits: Limits,
    pub env: BTreeMap<EnvKey, String>,
    pub labels: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lifecycle {
    // published on a localhost port and started again by podman when it exits
    Service { port: Port },
    // runs a command once, unpublished, and stays exited
    OneOff,
    // a one-off on a terminal with stdin open; attach before starting it so no output is missed
    Terminal,
    // a one-off with stdin open but no terminal, attached like `Terminal`; output comes framed
    Piped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    pub memory_bytes: u64,
    pub cpu_millicores: u32,
    pub pids: u32,
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
    pub oom_killed: bool,
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

    pub async fn pause_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}/pause", encode(id));
        let response = self.send(Method::POST, &path, DEFAULT_TIMEOUT).await?;
        check(response, || format!("container {id}"))?;
        Ok(())
    }

    pub async fn unpause_container(&self, id: &str) -> Result<()> {
        let path = format!("/containers/{}/unpause", encode(id));
        let response = self.send(Method::POST, &path, DEFAULT_TIMEOUT).await?;
        check(response, || format!("container {id}"))?;
        Ok(())
    }

    // the container's stdio from the start, until it exits: raw on a terminal, framed when piped
    pub async fn attach(&self, id: &str) -> Result<TokioIo<Upgraded>> {
        let path = format!(
            "/containers/{}/attach?stream=true&stdin=true&stdout=true&stderr=true",
            encode(id)
        );
        self.upgrade_without_body(&path).await
    }

    // only works while the container runs
    pub async fn resize_container(&self, id: &str, cols: u16, rows: u16) -> Result<()> {
        let path = format!("/containers/{}/resize?h={rows}&w={cols}", encode(id));
        let response = self.send(Method::POST, &path, DEFAULT_TIMEOUT).await?;
        check(response, || format!("container {id}"))?;
        Ok(())
    }

    // blocks until the container exits, so the caller picks how long to wait
    pub async fn wait_container(&self, id: &str, timeout: Duration) -> Result<i32> {
        let path = format!("/containers/{}/wait?condition=exited", encode(id));
        let response = self.send(Method::POST, &path, timeout).await?;
        let body = check(response, || format!("container {id}"))?;
        Ok(serde_json::from_slice(&body)?)
    }

    pub async fn remove_container(&self, id: &str) -> Result<()> {
        // volumes=true also drops anonymous volumes from VOLUME lines in the image; named ones stay
        let path = format!("/containers/{}?force=true&volumes=true", encode(id));
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
