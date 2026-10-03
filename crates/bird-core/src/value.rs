use std::fmt;
use std::num::NonZeroU16;
use std::str::FromStr;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ValidationError {
    #[error(
        "invalid name {0:?}: use 1-63 lowercase letters, digits or '-', starting with a letter"
    )]
    Name(String),
    #[error("invalid hostname {0:?}")]
    Hostname(String),
    #[error("invalid image reference {0:?}")]
    Image(String),
    #[error("invalid port {0}: must be 1-65535")]
    Port(u16),
    #[error(
        "invalid mount path {0:?}: use an absolute path like /var/lib/postgresql, without .. or spaces"
    )]
    MountPath(String),
    #[error("invalid command: give a program and up to 63 arguments, without NUL bytes")]
    Command,
    #[error(
        "invalid registry {0:?}: use a host like ghcr.io, docker.io or registry.example.com:5000"
    )]
    Registry(String),
    #[error("invalid memory limit {0:?}: use 32m to 256g, like 512m or 2g")]
    Memory(String),
    #[error("invalid cpu limit {0:?}: use 0.1 to 64 cores, like 0.5 or 2")]
    Cpu(String),
    #[error("invalid replica count: use 1-32 machines")]
    Replicas(u8),
    #[error("invalid variable key {0:?}: use letters, digits or '_', not starting with a digit")]
    EnvKey(String),
    #[error(
        "invalid dockerfile path {0:?}: use a path inside the build context like Dockerfile or docker/web.Dockerfile"
    )]
    BuildFile(String),
}

macro_rules! validated_string {
    ($name:ident, $parse:path, $variant:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
        #[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
        #[serde(try_from = "String", into = "String")]
        pub struct $name(String);

        impl $name {
            #[must_use]
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $name {
            type Error = ValidationError;

            fn try_from(value: String) -> Result<Self, Self::Error> {
                $parse(value).map(Self).map_err(ValidationError::$variant)
            }
        }

        impl FromStr for $name {
            type Err = ValidationError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Self::try_from(s.to_owned())
            }
        }

        impl From<$name> for String {
            fn from(value: $name) -> Self {
                value.0
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

validated_string!(Name, parse_name, Name);
validated_string!(Hostname, parse_hostname, Hostname);
validated_string!(ImageRef, parse_image, Image);
validated_string!(EnvKey, parse_env_key, EnvKey);
validated_string!(MountPath, parse_mount_path, MountPath);
validated_string!(RegistryHost, parse_registry_host, Registry);
validated_string!(BuildFile, parse_build_file, BuildFile);

fn parse_name(s: String) -> Result<String, String> {
    let b = s.as_bytes();
    let ok = matches!(b.first(), Some(b'a'..=b'z'))
        && b.len() <= 63
        && b.iter()
            .all(|c| matches!(c, b'a'..=b'z' | b'0'..=b'9' | b'-'))
        && b.last() != Some(&b'-');
    if ok { Ok(s) } else { Err(s) }
}

fn parse_hostname(s: String) -> Result<String, String> {
    let normalized = s.strip_suffix('.').unwrap_or(&s).to_ascii_lowercase();
    let ok = normalized.len() <= 253
        && normalized.split('.').count() >= 2
        && normalized.split('.').all(is_dns_label);
    if ok { Ok(normalized) } else { Err(s) }
}

fn is_dns_label(label: &str) -> bool {
    let b = label.as_bytes();
    !b.is_empty()
        && b.len() <= 63
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'-')
        && b.first() != Some(&b'-')
        && b.last() != Some(&b'-')
}

fn parse_image(s: String) -> Result<String, String> {
    let ok = !s.is_empty()
        && s.len() <= 512
        && s.bytes().next().is_some_and(|c| c.is_ascii_alphanumeric())
        && s.bytes().all(|c| {
            c.is_ascii_alphanumeric() || matches!(c, b'.' | b'_' | b'-' | b'/' | b':' | b'@')
        });
    if ok { Ok(s) } else { Err(s) }
}

fn parse_env_key(s: String) -> Result<String, String> {
    let b = s.as_bytes();
    let ok = b
        .first()
        .is_some_and(|c| c.is_ascii_alphabetic() || *c == b'_')
        && b.len() <= 255
        && b.iter().all(|c| c.is_ascii_alphanumeric() || *c == b'_');
    if ok { Ok(s) } else { Err(s) }
}

fn parse_registry_host(s: String) -> Result<String, String> {
    let normalized = s.trim().to_ascii_lowercase();
    let (host, port) = normalized.split_once(':').unwrap_or((&normalized, ""));
    let ok = !host.is_empty()
        && normalized.len() <= 253
        && (host.contains('.') || host == "localhost")
        && host.split('.').all(is_dns_label)
        && (port.is_empty() || port.parse::<u16>().is_ok_and(|p| p > 0));
    if ok { Ok(normalized) } else { Err(s) }
}

fn parse_mount_path(s: String) -> Result<String, String> {
    let ok = s.starts_with('/')
        && s.len() > 1
        && s.len() <= 255
        && s.split('/').all(|part| part != "..")
        && s.bytes()
            .all(|c| c.is_ascii_graphic() && c != b':' && c != b',');
    if ok { Ok(s) } else { Err(s) }
}

// relative to the build context and never leaving it
fn parse_build_file(s: String) -> Result<String, String> {
    let ok = !s.is_empty()
        && s.len() <= 255
        && !s.starts_with('/')
        && s.split('/')
            .all(|part| !part.is_empty() && part != ".." && part != ".")
        && s.bytes().all(|c| c.is_ascii_graphic());
    if ok { Ok(s) } else { Err(s) }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "openapi", derive(utoipa::ToSchema))]
#[cfg_attr(feature = "openapi", schema(value_type = u16, minimum = 1))]
#[serde(try_from = "u16", into = "u16")]
pub struct Port(NonZeroU16);

impl Port {
    pub const HTTP: Self = Self(NonZeroU16::MIN.saturating_add(79));

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0.get()
    }
}

impl TryFrom<u16> for Port {
    type Error = ValidationError;

    fn try_from(value: u16) -> Result<Self, Self::Error> {
        NonZeroU16::new(value)
            .map(Self)
            .ok_or(ValidationError::Port(value))
    }
}

impl From<Port> for u16 {
    fn from(value: Port) -> Self {
        value.get()
    }
}

impl fmt::Display for Port {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        for ok in ["a", "web", "my-app-2", &"a".repeat(63)] {
            assert!(ok.parse::<Name>().is_ok(), "{ok}");
        }
        for bad in [
            "",
            "Web",
            "2web",
            "-web",
            "web-",
            "my_app",
            "my.app",
            &"a".repeat(64),
        ] {
            assert!(bad.parse::<Name>().is_err(), "{bad}");
        }
    }

    #[test]
    fn hostnames_are_normalized() {
        let h: Hostname = "Web.Example.COM.".parse().unwrap();
        assert_eq!(h.as_str(), "web.example.com");
    }

    #[test]
    fn hostnames() {
        for ok in ["web.localhost", "a.b.c.example.com", "x-1.io"] {
            assert!(ok.parse::<Hostname>().is_ok(), "{ok}");
        }
        for bad in [
            "",
            "localhost",
            "-a.com",
            "a-.com",
            "a..com",
            "*.a.com",
            "a_b.com",
            "a b.com",
        ] {
            assert!(bad.parse::<Hostname>().is_err(), "{bad}");
        }
    }

    #[test]
    fn images() {
        for ok in [
            "nginx",
            "nginx:1.27",
            "ghcr.io/org/app:v1",
            "localhost:5000/app@sha256:abc123",
        ] {
            assert!(ok.parse::<ImageRef>().is_ok(), "{ok}");
        }
        for bad in ["", "-nginx", "--rm", "/nginx", "nginx latest", "nginx;rm"] {
            assert!(bad.parse::<ImageRef>().is_err(), "{bad}");
        }
    }

    #[test]
    fn build_files() {
        for ok in ["Dockerfile", "docker/web.Dockerfile", "Containerfile.prod"] {
            assert!(ok.parse::<BuildFile>().is_ok(), "{ok}");
        }
        for bad in [
            "",
            "/etc/passwd",
            "../Dockerfile",
            "a/../../b",
            "./Dockerfile",
            "a//b",
            "a b",
        ] {
            assert!(bad.parse::<BuildFile>().is_err(), "{bad}");
        }
    }

    #[test]
    fn env_keys() {
        for ok in ["PORT", "_X", "DATABASE_URL", "a1"] {
            assert!(ok.parse::<EnvKey>().is_ok(), "{ok}");
        }
        for bad in ["", "1A", "A-B", "A B", "A=B"] {
            assert!(bad.parse::<EnvKey>().is_err(), "{bad}");
        }
    }

    #[test]
    fn registry_hosts() {
        for ok in [
            "ghcr.io",
            "docker.io",
            "localhost:5000",
            "Registry.Example.com:443",
        ] {
            assert!(ok.parse::<RegistryHost>().is_ok(), "{ok}");
        }
        assert_eq!(
            "GHCR.io".parse::<RegistryHost>().unwrap().as_str(),
            "ghcr.io"
        );
        for bad in [
            "",
            "ghcr",
            "https://ghcr.io",
            "ghcr.io/org",
            "host:0",
            "host.io:99999",
        ] {
            assert!(bad.parse::<RegistryHost>().is_err(), "{bad}");
        }
    }

    #[test]
    fn mount_paths() {
        for ok in ["/data", "/var/lib/postgresql", "/srv/app-data_1"] {
            assert!(ok.parse::<MountPath>().is_ok(), "{ok}");
        }
        for bad in ["", "/", "data", "/a/../etc", "/a b", "/a:b", "/a,b"] {
            assert!(bad.parse::<MountPath>().is_err(), "{bad}");
        }
    }

    #[test]
    fn ports() {
        assert!(Port::try_from(0).is_err());
        assert_eq!(Port::try_from(8080).unwrap().get(), 8080);
        assert_eq!(Port::HTTP.get(), 80);
    }
}
