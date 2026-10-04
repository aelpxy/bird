use std::collections::BTreeMap;

use bird_core::EnvKey;
use serde::Serialize;

use super::{ContainerSpec, Lifecycle};

#[derive(Serialize)]
pub(super) struct SpecGenerator<'a> {
    name: &'a str,
    image: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    command: Option<&'a [String]>,
    env: &'a BTreeMap<EnvKey, String>,
    labels: &'a BTreeMap<String, String>,
    netns: Namespace,
    networks: BTreeMap<&'a str, NetworkOptions<'a>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    volumes: Vec<NamedVolume<'a>>,
    resource_limits: ResourceLimits,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    portmappings: Vec<PortMapping>,
    restart_policy: &'static str,
    // most apps ignore SIGTERM as pid 1, so stops would wait out the grace period and get killed
    init: bool,
}

#[derive(Serialize)]
struct Namespace {
    nsmode: &'static str,
}

#[derive(Serialize)]
struct NetworkOptions<'a> {
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    aliases: &'a [String],
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct NamedVolume<'a> {
    name: &'a str,
    dest: &'a str,
}

// cpu limits are a quota of runtime per scheduling period
const CPU_PERIOD_MICROS: u64 = 100_000;

#[derive(Serialize)]
struct ResourceLimits {
    memory: Limit<u64>,
    cpu: CpuQuota,
    pids: Limit<u32>,
}

#[derive(Serialize)]
struct Limit<T> {
    limit: T,
}

#[derive(Serialize)]
struct CpuQuota {
    quota: u64,
    period: u64,
}

#[derive(Serialize)]
struct PortMapping {
    container_port: u16,
    host_ip: &'static str,
    protocol: &'static str,
}

impl<'a> From<&'a ContainerSpec> for SpecGenerator<'a> {
    fn from(spec: &'a ContainerSpec) -> Self {
        Self {
            name: &spec.name,
            image: spec.image.qualified(),
            command: spec.command.as_deref(),
            env: &spec.env,
            labels: &spec.labels,
            netns: Namespace { nsmode: "bridge" },
            networks: BTreeMap::from([(
                spec.network.as_str(),
                NetworkOptions {
                    aliases: &spec.aliases,
                },
            )]),
            volumes: spec
                .mounts
                .iter()
                .map(|mount| NamedVolume {
                    name: &mount.volume,
                    dest: &mount.destination,
                })
                .collect(),
            resource_limits: ResourceLimits {
                memory: Limit {
                    limit: spec.limits.memory_bytes,
                },
                cpu: CpuQuota {
                    quota: u64::from(spec.limits.cpu_millicores) * CPU_PERIOD_MICROS / 1000,
                    period: CPU_PERIOD_MICROS,
                },
                pids: Limit {
                    limit: spec.limits.pids,
                },
            },
            portmappings: match spec.lifecycle {
                Lifecycle::Service { port } => vec![PortMapping {
                    container_port: port.get(),
                    host_ip: "127.0.0.1",
                    protocol: "tcp",
                }],
                Lifecycle::OneOff => Vec::new(),
            },
            restart_policy: match spec.lifecycle {
                Lifecycle::Service { .. } => "unless-stopped",
                Lifecycle::OneOff => "no",
            },
            init: true,
        }
    }
}

#[cfg(test)]
mod tests {
    use bird_core::Port;

    use super::*;

    fn spec(aliases: &[&str]) -> ContainerSpec {
        ContainerSpec {
            name: "bird-x".to_owned(),
            image: "nginx:alpine".parse().unwrap(),
            command: None,
            lifecycle: Lifecycle::Service {
                port: Port::try_from(80).unwrap(),
            },
            network: "bird".to_owned(),
            aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
            mounts: Vec::new(),
            limits: crate::Limits {
                memory_bytes: 536_870_912,
                cpu_millicores: 500,
                pids: 4096,
            },
            env: BTreeMap::from([("A".parse().unwrap(), "b".to_owned())]),
            labels: BTreeMap::new(),
        }
    }

    #[test]
    fn serializes_spec() {
        let spec = spec(&["web", "web.internal"]);
        let json = serde_json::to_value(SpecGenerator::from(&spec)).unwrap();
        assert_eq!(json["image"], "docker.io/library/nginx:alpine");
        assert!(json.get("command").is_none());
        assert_eq!(json["env"]["A"], "b");
        assert_eq!(
            json["networks"]["bird"]["aliases"],
            serde_json::json!(["web", "web.internal"])
        );
        assert_eq!(json["netns"]["nsmode"], "bridge");
        assert_eq!(json["init"], true);
        assert_eq!(json["portmappings"][0]["container_port"], 80);
        assert_eq!(json["portmappings"][0]["host_ip"], "127.0.0.1");
        assert_eq!(json["restart_policy"], "unless-stopped");
        assert_eq!(
            json["resource_limits"],
            serde_json::json!({
                "memory": {"limit": 536_870_912},
                "cpu": {"quota": 50_000, "period": 100_000},
                "pids": {"limit": 4096}
            })
        );
    }

    #[test]
    fn mounts_named_volumes() {
        let mut spec = spec(&[]);
        spec.mounts.push(crate::VolumeMount {
            volume: "bird-volume-1".to_owned(),
            destination: "/var/lib/postgresql".to_owned(),
        });
        let json = serde_json::to_value(SpecGenerator::from(&spec)).unwrap();
        assert_eq!(
            json["volumes"],
            serde_json::json!([{"Name": "bird-volume-1", "Dest": "/var/lib/postgresql"}])
        );
    }

    #[test]
    fn one_off_containers_are_unpublished_and_stay_exited() {
        let mut spec = spec(&[]);
        spec.lifecycle = Lifecycle::OneOff;
        let json = serde_json::to_value(SpecGenerator::from(&spec)).unwrap();
        assert!(json.get("portmappings").is_none());
        assert_eq!(json["restart_policy"], "no");
    }

    #[test]
    fn omits_empty_aliases() {
        let spec = spec(&[]);
        let json = serde_json::to_value(SpecGenerator::from(&spec)).unwrap();
        assert_eq!(json["networks"]["bird"], serde_json::json!({}));
        assert!(json.get("volumes").is_none());
    }
}
