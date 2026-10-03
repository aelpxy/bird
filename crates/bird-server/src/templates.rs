use bird_api::{DeployRequest, Manifest, TemplateSummary};
use bird_core::{EnvKey, ImageRef, Name};

use crate::{Error, Result};

const SERVICE_PLACEHOLDER: &str = "{service}";
const BUILT_IN: [&str; 2] = [
    include_str!("../templates/postgres.toml"),
    include_str!("../templates/valkey.toml"),
];

// a bird.toml manifest plus what the template list shows about it
#[derive(Debug, Clone)]
pub(crate) struct Template {
    description: String,
    connection: Option<EnvKey>,
    image: ImageRef,
    manifest: Manifest,
}

impl Template {
    fn parse(source: &str) -> Result<Self> {
        let invalid = |err: &dyn std::fmt::Display| Error::Template(err.to_string());
        let mut table: toml::Table = toml::from_str(source).map_err(|err| invalid(&err))?;
        let Some(toml::Value::String(description)) = table.remove("description") else {
            return Err(invalid(&"description must be a string"));
        };
        let connection = match table.remove("connection") {
            Some(toml::Value::String(key)) => Some(key.parse().map_err(|err| invalid(&err))?),
            Some(_) => return Err(invalid(&"connection must be a string")),
            None => None,
        };
        let manifest: Manifest = toml::Value::Table(table)
            .try_into()
            .map_err(|err| invalid(&err))?;
        let Some(image) = manifest.image.clone() else {
            return Err(invalid(&"templates need an image"));
        };
        if manifest.build.is_some() {
            return Err(invalid(&"templates cannot build from source"));
        }
        Ok(Self {
            description,
            connection,
            image,
            manifest,
        })
    }

    pub(crate) fn name(&self) -> &Name {
        &self.manifest.name
    }

    pub(crate) fn connection(&self) -> Option<&EnvKey> {
        self.connection.as_ref()
    }

    pub(crate) fn summary(&self) -> TemplateSummary {
        TemplateSummary {
            name: self.manifest.name.clone(),
            description: self.description.clone(),
            image: self.image.clone(),
        }
    }

    // {service} becomes the new service's name so urls point at its private address
    pub(crate) fn request(&self, service: &Name) -> DeployRequest {
        let mut manifest = self.manifest.clone();
        manifest.name = service.clone();
        for value in manifest.env.values_mut() {
            *value = value.replace(SERVICE_PLACEHOLDER, service.as_str());
        }
        manifest.into_request(self.image.clone())
    }
}

pub(crate) fn built_in() -> Result<Vec<Template>> {
    BUILT_IN
        .iter()
        .map(|source| Template::parse(source))
        .collect()
}

pub(crate) fn find(name: &Name) -> Result<Template> {
    built_in()?
        .into_iter()
        .find(|template| template.name() == name)
        .ok_or_else(|| Error::TemplateNotFound(name.clone()))
}

#[cfg(test)]
mod tests {
    use bird_core::{HealthCheck, reference};

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
            assert!(request.env.contains_key(connection), "{}", template.name());
            assert!(request.health.is_some(), "{}", template.name());
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
    fn rejects_unknown_fields() {
        let typo = "name = \"x\"\ndescription = \"d\"\nimage = \"x:1\"\nmemroy = \"1g\"\n";
        assert!(matches!(Template::parse(typo), Err(Error::Template(_))));
    }

    #[test]
    fn unknown_templates_are_reported() {
        assert!(matches!(
            find(&"mysql".parse().unwrap()),
            Err(Error::TemplateNotFound(_))
        ));
    }
}
