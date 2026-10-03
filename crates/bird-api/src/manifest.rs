use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use bird_core::{
    Command, CpuLimit, EnvKey, HealthCheck, Hostname, ImageRef, MemoryLimit, Name, Port,
};
use serde::{Deserialize, Deserializer};

use crate::{DeployRequest, VolumeSpec};

pub const MANIFEST_FILE: &str = "bird.toml";

// one service described as code, read from bird.toml and from the built-in templates
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: Name,
    pub image: ImageRef,
    #[serde(default)]
    pub port: Option<Port>,
    #[serde(default)]
    pub domains: Vec<Hostname>,
    #[serde(default)]
    pub health: Option<HealthCheck>,
    #[serde(default)]
    pub command: Option<Command>,
    #[serde(default, deserialize_with = "text_or_number")]
    pub memory: Option<MemoryLimit>,
    #[serde(default, deserialize_with = "text_or_number")]
    pub cpus: Option<CpuLimit>,
    #[serde(default)]
    pub env: BTreeMap<EnvKey, String>,
    #[serde(default)]
    pub volumes: Vec<VolumeSpec>,
}

impl Manifest {
    #[must_use]
    pub fn into_request(self) -> DeployRequest {
        DeployRequest {
            name: self.name,
            image: self.image,
            port: self.port.unwrap_or(Port::HTTP),
            domains: self.domains,
            env: self.env,
            health: self.health,
            command: self.command,
            memory: self.memory,
            cpus: self.cpus,
            volumes: self.volumes,
            allow_image_change: false,
        }
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum TextOrNumber {
    Text(String),
    Integer(i64),
    Float(f64),
}

// lets limits read naturally in toml: memory = "512m", cpus = 0.5
fn text_or_number<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    let raw = match TextOrNumber::deserialize(deserializer)? {
        TextOrNumber::Text(text) => text,
        TextOrNumber::Integer(number) => number.to_string(),
        TextOrNumber::Float(number) => number.to_string(),
    };
    raw.parse().map(Some).map_err(serde::de::Error::custom)
}
