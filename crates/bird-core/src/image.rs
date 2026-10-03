use std::fmt;

use crate::ImageRef;

impl ImageRef {
    // short names resolve to docker hub, the same way podman and docker resolve them
    #[must_use]
    pub fn qualified(&self) -> String {
        let raw = self.as_str();
        match raw.split_once('/') {
            None => format!("docker.io/library/{raw}"),
            Some((first, _)) if first.contains(['.', ':']) || first == "localhost" => {
                raw.to_owned()
            }
            Some(_) => format!("docker.io/{raw}"),
        }
    }

    // registries reject uppercase repositories, while hosts and tags may use any case
    #[must_use]
    pub fn has_lowercase_repository(&self) -> bool {
        let raw = self.as_str();
        let name = raw.split_once('@').map_or(raw, |(name, _)| name);
        let name_start = name.rfind('/').map_or(0, |i| i + 1);
        let name = match name.get(name_start..).and_then(|last| last.rfind(':')) {
            Some(colon) => name.get(..name_start + colon).unwrap_or(name),
            None => name,
        };
        let path = match name.split_once('/') {
            Some((first, rest)) if first.contains(['.', ':']) || first == "localhost" => rest,
            _ => name,
        };
        !path.bytes().any(|c| c.is_ascii_uppercase())
    }

    // images under localhost/ only exist in this server's podman, built by bird
    #[must_use]
    pub fn is_local(&self) -> bool {
        self.registry() == "localhost"
    }

    #[must_use]
    pub fn registry(&self) -> String {
        let qualified = self.qualified();
        qualified
            .split_once('/')
            .map_or(qualified.as_str(), |(host, _)| host)
            .to_ascii_lowercase()
    }
}

// what data on a volume depends on: the image, its major version and its base variant
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageLineage {
    repository: String,
    major: Option<String>,
    variant: Option<String>,
}

impl ImageLineage {
    #[must_use]
    pub fn of(image: &ImageRef) -> Self {
        let qualified = image.qualified();
        let without_digest = qualified
            .split_once('@')
            .map_or(qualified.as_str(), |(r, _)| r);
        let name_start = without_digest.rfind('/').map_or(0, |i| i + 1);
        let (repository, tag) = match without_digest.get(name_start..).and_then(|n| n.rfind(':')) {
            Some(colon) => without_digest.split_at(name_start + colon),
            None => (without_digest, ":latest"),
        };
        let tag = tag.trim_start_matches(':');
        let digits: String = tag.chars().take_while(char::is_ascii_digit).collect();
        let (major, variant) = if digits.is_empty() {
            (None, Some(tag).filter(|t| *t != "latest"))
        } else {
            (
                Some(digits),
                tag.split_once('-').map(|(_, variant)| variant),
            )
        };
        Self {
            repository: repository.to_owned(),
            major,
            variant: variant.map(str::to_owned),
        }
    }
}

impl fmt::Display for ImageLineage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}:{}",
            self.repository,
            self.major.as_deref().unwrap_or("*")
        )?;
        if let Some(variant) = &self.variant {
            write!(f, "-{variant}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lineage(raw: &str) -> String {
        ImageLineage::of(&raw.parse().unwrap()).to_string()
    }

    #[test]
    fn qualifies_short_names_to_docker_hub() {
        let qualify = |raw: &str| raw.parse::<ImageRef>().unwrap().qualified();
        assert_eq!(qualify("nginx"), "docker.io/library/nginx");
        assert_eq!(qualify("org/app:v1"), "docker.io/org/app:v1");
        assert_eq!(qualify("ghcr.io/org/app"), "ghcr.io/org/app");
        assert_eq!(qualify("localhost/app"), "localhost/app");
        assert_eq!(qualify("registry:5000/app"), "registry:5000/app");
    }

    #[test]
    fn checks_repository_case() {
        let lowercase = |raw: &str| raw.parse::<ImageRef>().unwrap().has_lowercase_repository();
        assert!(lowercase("nginx:1"));
        assert!(lowercase("Registry.Example.com:5000/app:V1-RC"));
        assert!(lowercase("localhost:5000/app@sha256:abc"));
        assert!(!lowercase("UPPER/Case"));
        assert!(!lowercase("Nginx:1"));
        assert!(!lowercase("ghcr.io/Org/app"));
    }

    #[test]
    fn finds_the_registry_host() {
        let registry = |raw: &str| raw.parse::<ImageRef>().unwrap().registry();
        assert_eq!(registry("nginx"), "docker.io");
        assert_eq!(registry("ghcr.io/org/app:1"), "ghcr.io");
        assert_eq!(registry("localhost:5000/private/app"), "localhost:5000");
        let local = |raw: &str| raw.parse::<ImageRef>().unwrap().is_local();
        assert!(local("localhost/bird/web:1"));
        assert!(!local("localhost:5000/web:1"));
        assert!(!local("nginx"));
    }

    #[test]
    fn minor_and_patch_versions_share_a_lineage() {
        assert_eq!(lineage("postgres:18"), "docker.io/library/postgres:18");
        assert_eq!(
            lineage("postgres:18.6"),
            lineage("docker.io/library/postgres:18")
        );
        assert_eq!(
            lineage("valkey/valkey:9.2-alpine"),
            lineage("valkey/valkey:9-alpine")
        );
    }

    #[test]
    fn majors_variants_and_repositories_differ() {
        assert_ne!(lineage("postgres:18"), lineage("postgres:19"));
        assert_ne!(lineage("postgres:18"), lineage("postgres:18-alpine"));
        assert_ne!(lineage("postgres:18"), lineage("ghcr.io/x/postgres:18"));
        assert_eq!(
            lineage("postgres:18-alpine"),
            "docker.io/library/postgres:18-alpine"
        );
    }

    #[test]
    fn handles_registry_ports_digests_and_untagged_images() {
        assert_eq!(lineage("registry:5000/db:2.1"), "registry:5000/db:2");
        assert_eq!(
            lineage("postgres:18@sha256:abc"),
            "docker.io/library/postgres:18"
        );
        assert_eq!(lineage("postgres"), "docker.io/library/postgres:*");
        assert_eq!(lineage("postgres:latest"), lineage("postgres"));
        assert_eq!(
            lineage("postgres:alpine"),
            "docker.io/library/postgres:*-alpine"
        );
    }
}
