use std::time::Duration;

use bird_core::ImageRef;
use hyper::{Method, StatusCode};
use serde::Deserialize;

use crate::error::check;
use crate::query::encode;
use crate::{Error, Podman, RegistryAuth, Result};

const PULL_TIMEOUT: Duration = Duration::from_secs(600);
// podman reports a missing tag and a registry blip alike, so every failed pull gets another go
const PULL_ATTEMPTS: u32 = 3;
const PULL_RETRY_DELAY: Duration = Duration::from_secs(2);

#[derive(Deserialize)]
struct PullReport {
    #[serde(default)]
    error: Option<String>,
    #[serde(default)]
    id: Option<String>,
}

impl Podman {
    pub async fn pull_image(
        &self,
        image: &ImageRef,
        auth: Option<&RegistryAuth>,
    ) -> Result<String> {
        let mut attempt = 1;
        loop {
            match self.pull_once(image, auth).await {
                Err(Error::Pull { image, message }) if attempt < PULL_ATTEMPTS => {
                    tracing::warn!(%image, attempt, error = %message, "image pull failed, retrying");
                    tokio::time::sleep(PULL_RETRY_DELAY * attempt).await;
                    attempt += 1;
                }
                result => return result,
            }
        }
    }

    async fn pull_once(&self, image: &ImageRef, auth: Option<&RegistryAuth>) -> Result<String> {
        let reference = image.qualified();
        let mut path = format!("/images/pull?reference={}&quiet=true", encode(&reference));
        let response = match auth {
            Some(auth) => {
                if !auth.tls_verify {
                    path.push_str("&tlsVerify=false");
                }
                self.send_with_registry_auth(Method::POST, &path, auth.header()?, PULL_TIMEOUT)
                    .await?
            }
            None => self.send(Method::POST, &path, PULL_TIMEOUT).await?,
        };
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

    pub async fn remove_image(&self, image: &ImageRef) -> Result<()> {
        let reference = image.qualified();
        let response = self
            .send(
                Method::DELETE,
                &format!("/images/{}", encode(&reference)),
                PULL_TIMEOUT,
            )
            .await?;
        check(response, || format!("image {reference}"))?;
        Ok(())
    }

    pub async fn image_exists(&self, image: &ImageRef) -> Result<bool> {
        let reference = image.qualified();
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
