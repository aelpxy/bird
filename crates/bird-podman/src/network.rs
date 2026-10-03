use hyper::{Method, StatusCode};
use serde::Serialize;

use crate::client::DEFAULT_TIMEOUT;
use crate::error::check;
use crate::query::encode;
use crate::{Error, Podman, Result};

#[derive(Serialize)]
struct CreateNetwork<'a> {
    name: &'a str,
    driver: &'a str,
    dns_enabled: bool,
}

impl Podman {
    pub async fn ensure_network(&self, name: &str) -> Result<()> {
        let subject = || format!("network {name}");
        let exists = self
            .get(&format!("/networks/{}/exists", encode(name)))
            .await?;
        if exists.status != StatusCode::NOT_FOUND {
            check(exists, subject)?;
            return Ok(());
        }

        let request = CreateNetwork {
            name,
            driver: "bridge",
            dns_enabled: true,
        };
        let response = self.post_json("/networks/create", &request).await?;
        match check(response, subject) {
            Ok(_) | Err(Error::Conflict { .. }) => Ok(()),
            Err(err) => Err(err),
        }
    }

    pub async fn remove_network(&self, name: &str) -> Result<()> {
        let response = self
            .send(
                Method::DELETE,
                &format!("/networks/{}", encode(name)),
                DEFAULT_TIMEOUT,
            )
            .await?;
        check(response, || format!("network {name}"))?;
        Ok(())
    }
}
