use std::time::Duration;

use bird_core::ImageRef;
use hyper::{Method, StatusCode};
use serde::Deserialize;

use crate::error::check;
use crate::query::encode;
use crate::{Error, Podman, Result};

const PULL_TIMEOUT: Duration = Duration::from_secs(600);

#[derive(Deserialize)]
struct PullReport {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

impl Podman {
    pub async fn pull_image(&self, image: &ImageRef) -> Result<String> {
        let reference = qualify_image(image);
        let path = format!("/images/pull?reference={}&quiet=true", encode(&reference));
        let response = self.send(Method::POST, &path, PULL_TIMEOUT).await?;
        let body = check(response, || format!("image {reference}"))?;

        let mut id = None;
        for report in serde_json::Deserializer::from_slice(&body).into_iter::<PullReport>() {
            let report = report?;
            if let Some(message) = report.error {
                return Err(Error::Pull {
                    image: reference,
                    message,
                });
            }
            if report.id.is_some() {
                id = report.id;
            }
        }
        id.ok_or_else(|| Error::Pull {
            image: reference,
            message: "response did not include an image id".to_owned(),
        })
    }

    pub async fn image_exists(&self, image: &ImageRef) -> Result<bool> {
        let reference = qualify_image(image);
        let response = self
            .get(&format!("/images/{}/exists", encode(&reference)))
            .await?;
        if response.status == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        check(response, || format!("image {reference}"))?;
        Ok(true)
    }
}

#[must_use]
pub fn qualify_image(image: &ImageRef) -> String {
    let raw = image.as_str();
    match raw.split_once('/') {
        None => format!("docker.io/library/{raw}"),
        Some((first, _)) if first.contains(['.', ':']) || first == "localhost" => raw.to_owned(),
        Some(_) => format!("docker.io/{raw}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn qualify(raw: &str) -> String {
        qualify_image(&raw.parse().unwrap())
    }

    #[test]
    fn qualifies_short_names_to_docker_hub() {
        assert_eq!(qualify("nginx"), "docker.io/library/nginx");
        assert_eq!(qualify("nginx:alpine"), "docker.io/library/nginx:alpine");
        assert_eq!(qualify("org/app:v1"), "docker.io/org/app:v1");
    }

    #[test]
    fn keeps_explicit_registries() {
        assert_eq!(qualify("ghcr.io/org/app"), "ghcr.io/org/app");
        assert_eq!(qualify("localhost/app"), "localhost/app");
        assert_eq!(qualify("registry:5000/app"), "registry:5000/app");
    }
}
