use std::collections::BTreeMap;

use bird_core::EnvKey;
use serde::Serialize;

use super::ContainerSpec;

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
    portmappings: [PortMapping; 1],
    restart_policy: &'static str,
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
            portmappings: [PortMapping {
                container_port: spec.port.get(),
                host_ip: "127.0.0.1",
                protocol: "tcp",
            }],
            restart_policy: "unless-stopped",
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
            port: Port::try_from(80).unwrap(),
            network: "bird".to_owned(),
            aliases: aliases.iter().map(|a| (*a).to_owned()).collect(),
            mounts: Vec::new(),
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
        assert_eq!(json["portmappings"][0]["container_port"], 80);
        assert_eq!(json["portmappings"][0]["host_ip"], "127.0.0.1");
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
    fn omits_empty_aliases() {
        let spec = spec(&[]);
        let json = serde_json::to_value(SpecGenerator::from(&spec)).unwrap();
        assert_eq!(json["networks"]["bird"], serde_json::json!({}));
        assert!(json.get("volumes").is_none());
    }
}
