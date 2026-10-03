use std::collections::BTreeMap;

use bird_api::{DeployRequest, TemplateSummary, VolumeSpec};
use bird_core::{Command, EnvKey, HealthCheck, ImageRef, Name, Port};
use serde::Deserialize;

use crate::{Error, Result};

const SERVICE_PLACEHOLDER: &str = "{service}";
const BUILT_IN: [&str; 2] = [
    include_str!("../templates/postgres.toml"),
    include_str!("../templates/valkey.toml"),
];

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Template {
    name: Name,
    description: String,
    image: ImageRef,
    port: Port,
    health: HealthCheck,
    #[serde(default)]
    connection: Option<EnvKey>,
    #[serde(default)]
    command: Option<Command>,
    #[serde(default)]
    volumes: Vec<VolumeSpec>,
    #[serde(default)]
    variables: BTreeMap<EnvKey, String>,
}

impl Template {
    pub(crate) fn name(&self) -> &Name {
        &self.name
    }

    pub(crate) fn connection(&self) -> Option<&EnvKey> {
        self.connection.as_ref()
    }

    pub(crate) fn summary(&self) -> TemplateSummary {
        TemplateSummary {
            name: self.name.clone(),
            description: self.description.clone(),
            image: self.image.clone(),
        }
    }

    // {service} becomes the new service's name so urls point at its private address
    pub(crate) fn request(&self, service: &Name) -> DeployRequest {
        let env = self
            .variables
            .iter()
            .map(|(key, value)| {
                (
                    key.clone(),
                    value.replace(SERVICE_PLACEHOLDER, service.as_str()),
                )
            })
            .collect();
        DeployRequest {
            name: service.clone(),
            image: self.image.clone(),
            port: self.port,
            domain: None,
            env,
            health: Some(self.health),
            command: self.command.clone(),
            memory: None,
            cpus: None,
            volumes: self.volumes.clone(),
            allow_image_change: false,
        }
    }
}

pub(crate) fn built_in() -> Result<Vec<Template>> {
    BUILT_IN
        .iter()
        .map(|source| toml::from_str(source).map_err(|err| Error::Template(err.to_string())))
        .collect()
}

pub(crate) fn find(name: &Name) -> Result<Template> {
    built_in()?
        .into_iter()
        .find(|template| &template.name == name)
        .ok_or_else(|| Error::TemplateNotFound(name.clone()))
}

#[cfg(test)]
mod tests {
    use bird_core::reference;

    use super::*;

    #[test]
    fn built_in_templates_parse_and_have_valid_references() {
        let templates = built_in().unwrap();
        assert_eq!(templates.len(), BUILT_IN.len());
        for template in &templates {
            let request = template.request(&"db".parse().unwrap());
            for value in request.env.values() {
                reference::validate(value).unwrap();
            }
            let connection = template.connection().unwrap();
            assert!(request.env.contains_key(connection), "{}", template.name);
            assert_ne!(request.volumes, Vec::new());
        }
    }

    #[test]
    fn urls_point_at_the_new_service() {
        let postgres = find(&"postgres".parse().unwrap()).unwrap();
        let request = postgres.request(&"maindb".parse().unwrap());
        let url = &request.env[&"DATABASE_URL".parse().unwrap()];
        assert!(url.contains("@maindb.internal:5432/"), "{url}");
        assert!(!url.contains(SERVICE_PLACEHOLDER));
        assert_eq!(request.health, Some(HealthCheck::Tcp));
    }

    #[test]
    fn valkey_url_names_the_default_user() {
        let valkey = find(&"valkey".parse().unwrap()).unwrap();
        let request = valkey.request(&"cache".parse().unwrap());
        let url = &request.env[&"VALKEY_URL".parse().unwrap()];
        assert!(url.starts_with("redis://default:"), "{url}");
    }

    #[test]
    fn unknown_templates_are_reported() {
        assert!(matches!(
            find(&"mysql".parse().unwrap()),
            Err(Error::TemplateNotFound(_))
        ));
    }
}
