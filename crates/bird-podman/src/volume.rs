use std::collections::BTreeMap;
use std::time::Duration;

use bytes::Bytes;
use hyper::body::{Body, Incoming};
use hyper::{Method, StatusCode};
use serde::Serialize;

use crate::client::DEFAULT_TIMEOUT;
use crate::error::check;
use crate::query::encode;
use crate::{Error, Podman, Result};

// podman answers an import only after reading the whole archive
const IMPORT_TIMEOUT: Duration = Duration::from_hours(1);

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

    // a tar of the volume's contents with owners and modes kept, streamed as podman reads it
    pub async fn export_volume(&self, name: &str) -> Result<Incoming> {
        let streamed = self
            .stream(&format!("/volumes/{}/export", encode(name)))
            .await?;
        if !streamed.status.is_success() {
            let status = streamed.status.as_u16();
            check(streamed.collect().await?, || format!("volume {name}"))?;
            return Err(Error::Api {
                status,
                message: "unexpected response to a volume export".to_owned(),
            });
        }
        Ok(streamed.body)
    }

    // extracts over what is there without deleting anything, so import into a fresh volume
    pub async fn import_volume<B>(&self, name: &str, archive: B) -> Result<()>
    where
        B: Body<Data = Bytes> + Send + 'static,
        B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    {
        let streamed = self
            .upload(
                &format!("/volumes/{}/import", encode(name)),
                "application/x-tar",
                archive,
                IMPORT_TIMEOUT,
            )
            .await?;
        check(streamed.collect().await?, || format!("volume {name}"))?;
        Ok(())
    }
}
