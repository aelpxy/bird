use std::collections::BTreeMap;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use bird_core::{
    BackupInterval, BackupKeep, BackupSchedule, BuildFile, Command, CpuLimit, EnvKey, HealthCheck,
    HealthTimeout, Hostname, ImageRef, MemoryLimit, Name, Port, Replicas,
};
use serde::{Deserialize, Deserializer};

use crate::{DeployRequest, VolumeSpec};

pub const MANIFEST_FILE: &str = "bird.toml";

// one service described as code, read from bird.toml and from the built-in templates
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub name: Name,
    /// Project and environment the CLI deploys to and acts in; flags override them
    #[serde(default)]
    pub project: Option<Name>,
    #[serde(default)]
    pub environment: Option<Name>,
    #[serde(default)]
    pub image: Option<ImageRef>,
    #[serde(default)]
    pub build: Option<BuildSpec>,
    #[serde(default)]
    pub port: Option<Port>,
    #[serde(default)]
    pub domains: Vec<Hostname>,
    #[serde(default)]
    pub health: Option<HealthCheck>,
    #[serde(default, deserialize_with = "text_or_number")]
    pub health_timeout: Option<HealthTimeout>,
    #[serde(default)]
    pub command: Option<Command>,
    #[serde(default, deserialize_with = "text_or_number")]
    pub memory: Option<MemoryLimit>,
    #[serde(default, deserialize_with = "text_or_number")]
    pub cpus: Option<CpuLimit>,
    #[serde(default)]
    pub replicas: Option<Replicas>,
    #[serde(default)]
    pub env: BTreeMap<EnvKey, String>,
    #[serde(default)]
    pub volumes: Vec<VolumeSpec>,
    #[serde(default)]
    pub backup: Option<BackupSpec>,
}

// `[backup] every = "1d"` backs the volumes up on a schedule, keeping the newest `keep` copies
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BackupSpec {
    #[serde(deserialize_with = "required_text_or_number")]
    pub every: BackupInterval,
    #[serde(default = "default_keep")]
    pub keep: BackupKeep,
}

const fn default_keep() -> BackupKeep {
    BackupKeep::WEEK
}

// builds the image from source on the server instead of pulling one
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BuildSpec {
    /// Directory sent to the server, relative to bird.toml; defaults to its directory
    #[serde(default)]
    pub context: Option<PathBuf>,
    /// Defaults to Dockerfile in the context
    #[serde(default)]
    pub dockerfile: Option<BuildFile>,
    /// Dockerfile ARG values; they stay readable in the image, so never put secrets here
    #[serde(default)]
    pub args: BTreeMap<EnvKey, String>,
}

impl Manifest {
    #[must_use]
    pub fn named(name: Name) -> Self {
        Self {
            name,
            project: None,
            environment: None,
            image: None,
            build: None,
            port: None,
            domains: Vec::new(),
            health: None,
            health_timeout: None,
            command: None,
            memory: None,
            cpus: None,
            replicas: None,
            env: BTreeMap::new(),
            volumes: Vec::new(),
            backup: None,
        }
    }

    // the image comes from the manifest, a flag or a build, so the caller settles it
    #[must_use]
    pub fn into_request(self, image: ImageRef) -> DeployRequest {
        DeployRequest {
            name: self.name,
            image,
            port: self.port.unwrap_or(Port::HTTP),
            domains: self.domains,
            env: self.env,
            health: self.health,
            health_timeout: self.health_timeout,
            command: self.command,
            memory: self.memory,
            cpus: self.cpus,
            volumes: self.volumes,
            replicas: self.replicas,
            backup: self.backup.map(|spec| BackupSchedule {
                every: spec.every,
                keep: spec.keep,
            }),
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

fn required_text_or_number<'de, D, T>(deserializer: D) -> Result<T, D::Error>
where
    D: Deserializer<'de>,
    T: FromStr,
    T::Err: fmt::Display,
{
    text_or_number(deserializer)?.ok_or_else(|| serde::de::Error::custom("a value is required"))
}
