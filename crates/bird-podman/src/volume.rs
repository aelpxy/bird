use std::collections::BTreeMap;

use hyper::{Method, StatusCode};
use serde::Serialize;

use crate::client::DEFAULT_TIMEOUT;
use crate::error::check;
use crate::query::encode;
use crate::{Error, Podman, Result};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VolumeMount {
    pub volume: String,
    pub destination: String,
}

#[derive(Serialize)]
#[serde(rename_all = "PascalCase")]
struct CreateVolume<'a> {
    name: &'a str,
    labels: &'a BTreeMap<String, String>,
}

impl Podman {
    pub async fn ensure_volume(&self, name: &str, labels: &BTreeMap<String, String>) -> Result<()> {
        let subject = || format!("volume {name}");
        let exists = self
            .get(&format!("/volumes/{}/exists", encode(name)))
            .await?;
        if exists.status != StatusCode::NOT_FOUND {
            check(exists, subject)?;
            return Ok(());
        }
        let response = self
            .post_json("/volumes/create", &CreateVolume { name, labels })
            .await?;
        match check(response, subject) {
            Ok(_) | Err(Error::Conflict { .. }) => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub async fn volume_exists(&self, name: &str) -> Result<bool> {
        let response = self
            .get(&format!("/volumes/{}/exists", encode(name)))
            .await?;
        if response.status == StatusCode::NOT_FOUND {
            return Ok(false);
        }
        check(response, || format!("volume {name}"))?;
        Ok(true)
    }

    pub async fn remove_volume(&self, name: &str) -> Result<()> {
        let response = self
            .send(
                Method::DELETE,
                &format!("/volumes/{}", encode(name)),
                DEFAULT_TIMEOUT,
            )
            .await?;
        check(response, || format!("volume {name}"))?;
        Ok(())
    }
}
